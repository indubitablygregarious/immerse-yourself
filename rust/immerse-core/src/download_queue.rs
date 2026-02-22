//! Background download queue for freesound.org sounds.
//!
//! Downloads are processed one at a time in background threads to avoid blocking the UI.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use regex::Regex;
use reqwest::blocking::Client;
use serde_json;

/// Download status for UI updates.
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    /// Download is queued, waiting to start
    Queued,
    /// Download is in progress
    Downloading { display_name: String },
    /// Download completed successfully
    Complete { local_path: PathBuf },
    /// Download failed
    Failed { error: String },
}

/// Callback type for download completion.
pub type DownloadCallback = Box<dyn FnOnce(Result<PathBuf, String>) + Send + 'static>;

/// A single download request.
struct DownloadRequest {
    url: String,
    callback: Option<DownloadCallback>,
}

/// Download queue manager for freesound.org sounds.
///
/// Downloads are queued and processed one at a time in background threads.
pub struct DownloadQueue {
    cache_dir: PathBuf,
    queue: Arc<Mutex<VecDeque<DownloadRequest>>>,
    pending_urls: Arc<RwLock<HashSet<String>>>,
    status_map: Arc<RwLock<HashMap<String, DownloadStatus>>>,
    is_processing: Arc<Mutex<bool>>,
    /// Sound manifest mapping freesound URLs to local file paths (bundled sounds).
    manifest: Arc<RwLock<HashMap<String, PathBuf>>>,
    /// Whether on-demand downloads are enabled. When false, enqueue() skips downloading.
    downloads_enabled: Arc<RwLock<bool>>,
}

impl DownloadQueue {
    /// Creates a new download queue.
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Self {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&cache_dir);

        Self {
            cache_dir,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            pending_urls: Arc::new(RwLock::new(HashSet::new())),
            status_map: Arc::new(RwLock::new(HashMap::new())),
            is_processing: Arc::new(Mutex::new(false)),
            manifest: Arc::new(RwLock::new(HashMap::new())),
            downloads_enabled: Arc::new(RwLock::new(false)),
        }
    }

    /// Enqueues a URL for download.
    ///
    /// Returns immediately. The callback will be called when the download completes.
    /// If the file is already cached, the callback is called immediately with the cached path.
    pub fn enqueue<F>(&self, url: &str, callback: F) -> bool
    where
        F: FnOnce(Result<PathBuf, String>) + Send + 'static,
    {
        // Check if already cached (manifest + cache dir)
        if let Some(cached) = self.find_cached(url) {
            tracing::info!("Sound already cached: {}", url);
            callback(Ok(cached));
            return false; // Not queued, already had it
        }

        // If downloads are disabled, don't queue
        {
            let enabled = self.downloads_enabled.read().unwrap();
            if !*enabled {
                tracing::debug!("Downloads disabled, skipping: {}", url);
                callback(Err("Downloads disabled".into()));
                return false;
            }
        }

        // Check if already in queue
        {
            let pending = self.pending_urls.read().unwrap();
            if pending.contains(url) {
                tracing::debug!("URL already in download queue: {}", url);
                return false;
            }
        }

        // Add to pending set
        {
            let mut pending = self.pending_urls.write().unwrap();
            pending.insert(url.to_string());
        }

        // Update status
        {
            let mut status = self.status_map.write().unwrap();
            status.insert(url.to_string(), DownloadStatus::Queued);
        }

        // Add to queue
        {
            let mut queue = self.queue.lock().unwrap();
            queue.push_back(DownloadRequest {
                url: url.to_string(),
                callback: Some(Box::new(callback)),
            });
        }

        tracing::info!("Queued download: {}", url);

        // Start processing if not already running
        self.start_processing();

        true
    }

    /// Enqueues a URL without a callback, returning the cached path if available.
    ///
    /// If not cached, returns None and queues the download.
    pub fn enqueue_or_get_cached(&self, url: &str) -> Option<PathBuf> {
        // Check if already cached
        if let Some(cached) = self.find_cached(url) {
            return Some(cached);
        }

        // Queue it (with no-op callback)
        self.enqueue(url, |_| {});
        None
    }

    /// Gets the current status of a URL.
    pub fn get_status(&self, url: &str) -> Option<DownloadStatus> {
        let status = self.status_map.read().unwrap();
        status.get(url).cloned()
    }

    /// Gets the number of pending downloads.
    pub fn pending_count(&self) -> usize {
        let pending = self.pending_urls.read().unwrap();
        pending.len()
    }

    /// Checks if a URL is currently being downloaded.
    pub fn is_downloading(&self, url: &str) -> bool {
        let pending = self.pending_urls.read().unwrap();
        pending.contains(url)
    }

    /// Gets all currently downloading URLs.
    pub fn get_downloading_urls(&self) -> Vec<String> {
        let pending = self.pending_urls.read().unwrap();
        pending.iter().cloned().collect()
    }

    /// Public version of find_cached for external use.
    pub fn find_cached_public(&self, url: &str) -> Option<PathBuf> {
        self.find_cached(url)
    }

    /// Loads a sound manifest mapping freesound URLs to local file paths.
    ///
    /// The manifest JSON maps URLs to relative paths (e.g., "freesound_sounds/cc0/file.mp3").
    /// The `base_dir` is used to resolve relative paths to absolute paths.
    pub fn load_manifest(&self, base_dir: &Path, manifest_path: &Path) {
        match std::fs::read_to_string(manifest_path) {
            Ok(contents) => {
                match serde_json::from_str::<HashMap<String, String>>(&contents) {
                    Ok(raw_manifest) => {
                        let mut manifest = self.manifest.write().unwrap();
                        let mut count = 0;
                        for (url, rel_path) in raw_manifest {
                            let abs_path = base_dir.join(&rel_path);
                            if abs_path.exists() {
                                manifest.insert(url, abs_path);
                                count += 1;
                            }
                        }
                        tracing::info!("Loaded sound manifest: {} entries from {:?}", count, manifest_path);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse sound manifest {:?}: {}", manifest_path, e);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("No sound manifest at {:?}: {}", manifest_path, e);
            }
        }
    }

    /// Returns the number of entries in the sound manifest.
    pub fn manifest_size(&self) -> usize {
        self.manifest.read().unwrap().len()
    }

    /// Returns a snapshot of the current manifest (URL → absolute path).
    pub fn get_manifest(&self) -> HashMap<String, PathBuf> {
        self.manifest.read().unwrap().clone()
    }

    /// No-op: runtime downloads are permanently disabled.
    /// All sounds must be pre-packaged in the freesound_sounds directory.
    pub fn set_downloads_enabled(&self, _enabled: bool) {
        tracing::warn!("set_downloads_enabled() called but runtime downloads are permanently disabled");
    }

    /// Returns whether on-demand downloads are enabled.
    pub fn downloads_enabled(&self) -> bool {
        *self.downloads_enabled.read().unwrap()
    }

    /// Finds a cached file for a freesound URL.
    fn find_cached(&self, url: &str) -> Option<PathBuf> {
        // Check bundled sound manifest first
        {
            let manifest = self.manifest.read().unwrap();
            if let Some(path) = manifest.get(url) {
                if path.exists() {
                    tracing::debug!("Found bundled sound for {}", url);
                    return Some(path.clone());
                }
            }
        }

        let (creator, sound_id) = match parse_freesound_url(url) {
            Some(parsed) => parsed,
            None => return None,
        };

        // Look for files matching creator_id_*
        if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("{}_{}_", creator, sound_id)) {
                    return Some(entry.path());
                }
            }
        }

        None
    }

    /// Starts the background processing thread if not already running.
    fn start_processing(&self) {
        let mut is_processing = self.is_processing.lock().unwrap();
        if *is_processing {
            return;
        }
        *is_processing = true;
        drop(is_processing);

        let queue = Arc::clone(&self.queue);
        let pending_urls = Arc::clone(&self.pending_urls);
        let status_map = Arc::clone(&self.status_map);
        let is_processing = Arc::clone(&self.is_processing);
        let cache_dir = self.cache_dir.clone();

        thread::spawn(move || {
            loop {
                // Get next request
                let request = {
                    let mut queue = queue.lock().unwrap();
                    queue.pop_front()
                };

                let Some(request) = request else {
                    // Queue is empty, stop processing
                    let mut is_processing = is_processing.lock().unwrap();
                    *is_processing = false;
                    break;
                };

                let url = request.url.clone();

                // Update status to downloading
                {
                    let display_name = extract_display_name(&url);
                    let mut status = status_map.write().unwrap();
                    status.insert(url.clone(), DownloadStatus::Downloading { display_name });
                }

                // Perform download
                let result = download_sound(&url, &cache_dir);

                // Update status and remove from pending
                {
                    let mut status = status_map.write().unwrap();
                    let mut pending = pending_urls.write().unwrap();

                    match &result {
                        Ok(path) => {
                            status.insert(url.clone(), DownloadStatus::Complete {
                                local_path: path.clone(),
                            });
                        }
                        Err(e) => {
                            status.insert(url.clone(), DownloadStatus::Failed {
                                error: e.clone(),
                            });
                        }
                    }

                    pending.remove(&url);
                }

                // Call the callback
                if let Some(callback) = request.callback {
                    callback(result);
                }
            }
        });
    }
}

/// Parses a freesound.org URL to extract creator and sound ID.
pub fn parse_freesound_url(url: &str) -> Option<(String, String)> {
    let re = Regex::new(r"freesound\.org/people/([^/]+)/sounds/(\d+)").ok()?;
    let caps = re.captures(url)?;
    Some((caps[1].to_string(), caps[2].to_string()))
}

/// Extracts a display name from a freesound URL.
fn extract_display_name(url: &str) -> String {
    if let Some((_, sound_id)) = parse_freesound_url(url) {
        format!("Sound {}", sound_id)
    } else {
        "Unknown".to_string()
    }
}

/// Downloads a sound from freesound.org.
pub fn download_sound(url: &str, cache_dir: &Path) -> Result<PathBuf, String> {
    let (creator, sound_id) = parse_freesound_url(url)
        .ok_or_else(|| format!("Invalid freesound URL: {}", url))?;

    tracing::info!("Downloading sound: {} ({})", url, sound_id);

    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Fetch page HTML once and extract both name and audio URL
    let html = client.get(url).send()
        .map_err(|e| format!("Failed to fetch page: {}", e))?
        .text()
        .map_err(|e| format!("Failed to read page body: {}", e))?;

    let sound_name = extract_sound_name_from_html(&html)
        .unwrap_or_else(|| format!("sound_{}", sound_id));
    let audio_url = extract_audio_url_from_html(&html, url)?;

    // Extract the real file extension from the audio URL (e.g., .wav, .flac, .ogg)
    // instead of hardcoding .mp3 — symphonia uses extension as a codec hint.
    let extension = audio_url
        .rsplit('/')
        .next()
        .and_then(|filename| filename.split('?').next())
        .and_then(|clean| clean.rsplit('.').next())
        .and_then(|ext| match ext.to_lowercase().as_str() {
            "mp3" | "wav" | "flac" | "ogg" | "opus" => Some(ext.to_lowercase()),
            _ => None,
        })
        .unwrap_or_else(|| "mp3".to_string());

    let output_path = cache_dir.join(format!("{}_{}_{}.{}", creator, sound_id, sound_name, extension));

    // Download audio file
    let bytes = client.get(&audio_url).send()
        .map_err(|e| format!("Failed to download audio: {}", e))?
        .bytes()
        .map_err(|e| format!("Failed to read audio bytes: {}", e))?;

    if bytes.is_empty() {
        return Err("Downloaded file is empty".to_string());
    }

    std::fs::write(&output_path, &bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    tracing::info!("Downloaded: {:?}", output_path);
    Ok(output_path)
}

/// Extracts the actual audio URL from freesound.org page HTML.
fn extract_audio_url_from_html(html: &str, page_url: &str) -> Result<String, String> {
    // Look for twitter:player:stream meta tag
    if let Some(start) = html.find("twitter:player:stream") {
        if let Some(content_start) = html[start..].find("content=\"") {
            let url_start = start + content_start + 9;
            if let Some(url_end) = html[url_start..].find('"') {
                let audio_url = &html[url_start..url_start + url_end];
                if audio_url.starts_with("https://") {
                    return Ok(audio_url.to_string());
                }
            }
        }
    }

    // Fallback: try og:audio
    if let Some(start) = html.find("og:audio") {
        if let Some(content_start) = html[start..].find("content=\"") {
            let url_start = start + content_start + 9;
            if let Some(url_end) = html[url_start..].find('"') {
                let audio_url = &html[url_start..url_start + url_end];
                if let Some(cdn_start) = audio_url.find("https://cdn.freesound.org") {
                    return Ok(audio_url[cdn_start..].to_string());
                }
            }
        }
    }

    Err(format!("Could not extract audio URL from: {}", page_url))
}

/// Extracts the sound name from freesound.org page HTML.
fn extract_sound_name_from_html(html: &str) -> Option<String> {
    let re = Regex::new(r"<title>([^<]+)</title>").ok()?;
    let caps = re.captures(html)?;
    let title = caps[1].trim();

    // Remove " - Freesound" suffix
    let name = if title.contains(" - Freesound") {
        title.replace(" - Freesound", "")
    } else {
        title.to_string()
    };

    Some(sanitize_filename(&name))
}

/// Sanitizes a string for use as a filename.
fn sanitize_filename(name: &str) -> String {
    let mut result = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            ' ' => '_',
            c => c,
        })
        .collect::<String>();

    // Collapse multiple underscores
    while result.contains("__") {
        result = result.replace("__", "_");
    }

    // Remove leading/trailing underscores
    result.trim_matches('_').to_string()
}

/// Finds a downloaded file matching creator and sound ID.
pub fn find_downloaded_file(cache_dir: &Path, creator: &str, sound_id: &str) -> Option<PathBuf> {
    let prefix = format!("{}_{}_", creator, sound_id);
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Loads a sound manifest file and returns a map of freesound URLs to absolute file paths.
///
/// This is a standalone function (not tied to DownloadQueue) so it can be called early
/// during initialization, before engines are created.
pub fn load_sound_manifest(base_dir: &Path, manifest_path: &Path) -> HashMap<String, PathBuf> {
    let mut result = HashMap::new();
    let contents = match std::fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(_) => return result,
    };
    let raw: HashMap<String, String> = match serde_json::from_str(&contents) {
        Ok(m) => m,
        Err(_) => return result,
    };
    for (url, rel_path) in raw {
        let abs_path = base_dir.join(&rel_path);
        if abs_path.exists() {
            result.insert(url, abs_path);
        }
    }
    result
}

/// Checks if a URL is a valid freesound.org URL.
pub fn is_freesound_url(url: &str) -> bool {
    parse_freesound_url(url).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_freesound_url() {
        let url = "https://freesound.org/people/klankbeeld/sounds/625333/";
        let parsed = parse_freesound_url(url);
        assert_eq!(parsed, Some(("klankbeeld".to_string(), "625333".to_string())));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Hello World"), "Hello_World");
        assert_eq!(sanitize_filename("Test: File?"), "Test_File");
        assert_eq!(sanitize_filename("  spaces  "), "spaces");
    }

    #[test]
    fn test_is_freesound_url() {
        assert!(is_freesound_url("https://freesound.org/people/user/sounds/12345/"));
        assert!(!is_freesound_url("https://example.com/sound.mp3"));
    }
}
