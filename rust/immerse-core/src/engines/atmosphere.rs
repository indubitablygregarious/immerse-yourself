//! Atmosphere engine for ambient sound playback from freesound.org URLs.
//!
//! All atmosphere sounds play through a single kira `AudioManager` (one cpal
//! stream). Each sound gets its own `StaticSoundHandle` with per-sound volume
//! control via kira's internal mixer. No background stream-keeper threads needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::PlaybackState;
use kira::Tween;

use rand::Rng;

use crate::download_queue::DownloadQueue;
use crate::engines::audio_output::{volume_to_db, with_audio_manager};
use crate::error::{Error, Result};

/// Configuration for a pool of mutually exclusive sounds.
struct PoolConfig {
    /// All URLs in this pool.
    urls: Vec<String>,
    /// Volume for each URL.
    volumes: HashMap<String, u8>,
}

/// Atmosphere engine for playing looping ambient sounds.
pub struct AtmosphereEngine {
    cache_dir: PathBuf,
    active_sounds: Arc<Mutex<HashMap<String, ActiveSound>>>,
    download_queue: Arc<DownloadQueue>,
    /// Generation counter - incremented on stop_all() to invalidate pending download callbacks.
    generation: Arc<AtomicU64>,
    /// Active sound pools — keyed by pool name. Monitor threads exit when their pool is removed.
    active_pools: Arc<Mutex<HashMap<String, PoolConfig>>>,
}

/// A playing atmosphere sound backed by a kira StaticSoundHandle.
struct ActiveSound {
    handle: StaticSoundHandle,
    volume: u8,
}

impl AtmosphereEngine {
    /// Creates a new atmosphere engine with cache dir at `project_root/freesound.org/`.
    pub fn new<P: AsRef<Path>>(project_root: P) -> Self {
        let cache_dir = project_root.as_ref().join("freesound.org");
        Self::new_with_cache_dir(cache_dir)
    }

    /// Creates a new atmosphere engine with an explicit cache directory.
    /// Use this on iOS where the project root is read-only and downloads
    /// must go to a writable location (e.g. `app_cache_dir/freesound.org/`).
    pub fn new_with_cache_dir<P: AsRef<Path>>(cache_dir: P) -> Self {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        // Ensure cache directory exists
        let _ = std::fs::create_dir_all(&cache_dir);

        // Create download queue using the same cache dir
        let download_queue = Arc::new(DownloadQueue::new(&cache_dir));

        Self {
            cache_dir,
            active_sounds: Arc::new(Mutex::new(HashMap::new())),
            download_queue,
            generation: Arc::new(AtomicU64::new(0)),
            active_pools: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Starts playing a single sound.
    pub fn start_single(&self, url: &str, volume: u8) -> Result<()> {
        self.start_single_with_options(url, volume, None, None)
    }

    /// Starts playing a single sound with optional fade-out duration.
    pub fn start_single_with_duration(&self, url: &str, volume: u8, fade_duration: Option<u32>) -> Result<()> {
        self.start_single_with_options(url, volume, fade_duration, None)
    }

    /// Starts playing a single sound with optional max duration.
    pub fn start_single_with_max_duration(&self, url: &str, volume: u8, max_duration: Option<u32>) -> Result<()> {
        self.start_single_with_options(url, volume, None, max_duration)
    }

    /// Starts playing a single sound with optional fade-out and/or max duration.
    pub fn start_single_with_options(&self, url: &str, volume: u8, fade_duration: Option<u32>, max_duration: Option<u32>) -> Result<()> {
        // Check if already playing
        {
            let sounds = self.active_sounds.lock().map_err(|_| {
                Error::AtmospherePlayback("Failed to acquire lock".to_string())
            })?;
            if sounds.contains_key(url) {
                return Ok(()); // Already playing
            }
        }

        // Capture current generation
        let start_generation = self.generation.load(Ordering::SeqCst);

        let url_owned = url.to_string();
        let active_sounds = Arc::clone(&self.active_sounds);
        let generation = Arc::clone(&self.generation);

        // Check if cached first
        if let Some(cached_path) = self.download_queue.enqueue_or_get_cached(url) {
            return start_playback_internal(&url_owned, &cached_path, volume, &active_sounds, true, fade_duration, max_duration);
        }

        // Not cached - queue download with callback to start playback
        let volume_copy = volume;
        self.download_queue.enqueue(url, move |result| {
            let current_generation = generation.load(Ordering::SeqCst);
            if current_generation != start_generation {
                tracing::info!(
                    "Skipping atmosphere sound {} - generation changed ({} -> {}), environment was switched",
                    url_owned, start_generation, current_generation
                );
                return;
            }

            match result {
                Ok(path) => {
                    if let Err(e) = start_playback_internal(&url_owned, &path, volume_copy, &active_sounds, true, fade_duration, max_duration) {
                        tracing::warn!("Failed to start atmosphere sound after download: {}", e);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to download atmosphere sound {}: {}", url_owned, e);
                }
            }
        });

        tracing::info!("Queued atmosphere sound for download: {} (generation {})", url, start_generation);
        Ok(())
    }

    /// Registers a pool of mutually exclusive sounds. Call `start_pool` afterward to begin playback.
    pub fn register_pool(&self, pool_name: &str, sounds: Vec<(String, u8)>) {
        let mut volumes = HashMap::new();
        let mut urls = Vec::new();
        for (url, vol) in sounds {
            volumes.insert(url.clone(), vol);
            urls.push(url);
        }
        let count = urls.len();
        if let Ok(mut pools) = self.active_pools.lock() {
            pools.insert(pool_name.to_string(), PoolConfig { urls, volumes });
        }
        tracing::info!("Registered pool '{}' with {} sounds", pool_name, count);
    }

    /// Starts a pool: picks a random sound, plays it (non-looping), and spawns a monitor
    /// thread that starts the next sound when the current one finishes.
    pub fn start_pool(&self, pool_name: &str) -> Result<()> {
        let (url, volume) = {
            let pools = self.active_pools.lock().map_err(|_| {
                Error::AtmospherePlayback("Failed to acquire pool lock".to_string())
            })?;
            let pool = pools.get(pool_name).ok_or_else(|| {
                Error::AtmospherePlayback(format!("Pool '{}' not registered", pool_name))
            })?;
            let mut rng = rand::thread_rng();
            let idx = rng.gen_range(0..pool.urls.len());
            let url = pool.urls[idx].clone();
            let volume = pool.volumes.get(&url).copied().unwrap_or(70);
            (url, volume)
        };

        // Start the first track (non-looping)
        self.start_single_no_loop(&url, volume)?;

        // Spawn monitor thread
        let pool_name = pool_name.to_string();
        let active_sounds = Arc::clone(&self.active_sounds);
        let active_pools = Arc::clone(&self.active_pools);
        let generation = Arc::clone(&self.generation);
        let download_queue = Arc::clone(&self.download_queue);
        let start_generation = self.generation.load(Ordering::SeqCst);
        let current_url = url;

        std::thread::Builder::new()
            .name(format!("pool-monitor-{}", pool_name))
            .spawn(move || {
                pool_monitor_loop(
                    &pool_name,
                    current_url,
                    start_generation,
                    &active_sounds,
                    &active_pools,
                    &generation,
                    &download_queue,
                );
            })
            .map_err(|e| Error::AtmospherePlayback(format!("Failed to spawn pool monitor: {}", e)))?;

        Ok(())
    }

    /// Starts a single sound without looping (plays once then stops).
    /// Used for pool sounds — the pool monitor detects when it finishes and starts the next one.
    fn start_single_no_loop(&self, url: &str, volume: u8) -> Result<()> {
        // Check if already playing
        {
            let sounds = self.active_sounds.lock().map_err(|_| {
                Error::AtmospherePlayback("Failed to acquire lock".to_string())
            })?;
            if sounds.contains_key(url) {
                return Ok(());
            }
        }

        let url_owned = url.to_string();
        let active_sounds = Arc::clone(&self.active_sounds);

        // Must be cached (pool sounds are pre-bundled)
        if let Some(cached_path) = self.download_queue.enqueue_or_get_cached(url) {
            return start_playback_internal(&url_owned, &cached_path, volume, &active_sounds, false, None, None);
        }

        // Queue download as fallback
        let start_generation = self.generation.load(Ordering::SeqCst);
        let generation = Arc::clone(&self.generation);
        self.download_queue.enqueue(url, move |result| {
            let current_generation = generation.load(Ordering::SeqCst);
            if current_generation != start_generation {
                return;
            }
            if let Ok(path) = result {
                let _ = start_playback_internal(&url_owned, &path, volume, &active_sounds, false, None, None);
            }
        });

        Ok(())
    }

    /// Stops a single sound.
    pub fn stop_single(&self, url: &str) -> Result<()> {
        let mut sounds = self.active_sounds.lock().map_err(|_| {
            Error::AtmospherePlayback("Failed to acquire lock".to_string())
        })?;

        if let Some(mut active) = sounds.remove(url) {
            active.handle.stop(Tween::default());
            tracing::info!("Stopped atmosphere sound: {}", url);
        }

        Ok(())
    }

    /// Stops all playing sounds and invalidates pending download callbacks.
    /// Also clears all active pools so monitor threads exit.
    pub fn stop_all(&self) -> usize {
        let old_gen = self.generation.fetch_add(1, Ordering::SeqCst);
        tracing::info!("stop_all: incremented generation from {} to {}", old_gen, old_gen + 1);

        // Clear pools first so monitor threads see the pool gone and exit
        if let Ok(mut pools) = self.active_pools.lock() {
            pools.clear();
        }

        let mut count = 0;

        if let Ok(mut sounds) = self.active_sounds.lock() {
            for (url, mut active) in sounds.drain() {
                active.handle.stop(Tween::default());
                count += 1;
                tracing::info!("Stopped atmosphere sound: {}", url);
            }
        }

        count
    }

    /// Stops all playing sounds EXCEPT those in the keep set.
    /// Returns the number of sounds stopped.
    /// Also removes pools whose URLs are not in the keep set.
    pub fn stop_all_except(&self, keep_urls: &std::collections::HashSet<String>) -> usize {
        self.generation.fetch_add(1, Ordering::SeqCst);

        // Remove pools whose URLs aren't all in the keep set
        if let Ok(mut pools) = self.active_pools.lock() {
            let pools_to_remove: Vec<String> = pools.iter()
                .filter(|(_, pool)| !pool.urls.iter().any(|u| keep_urls.contains(u)))
                .map(|(name, _)| name.clone())
                .collect();
            for name in pools_to_remove {
                pools.remove(&name);
                tracing::info!("Removed pool '{}' (not in keep set)", name);
            }
        }

        let mut count = 0;
        if let Ok(mut sounds) = self.active_sounds.lock() {
            let to_remove: Vec<String> = sounds.keys()
                .filter(|u| !keep_urls.contains(*u))
                .cloned()
                .collect();
            for url in to_remove {
                if let Some(mut active) = sounds.remove(&url) {
                    active.handle.stop(Tween::default());
                    count += 1;
                    tracing::info!("Stopped atmosphere sound: {}", url);
                }
            }
        }
        count
    }

    /// Sets the volume for a playing sound.
    pub fn set_volume(&self, url: &str, volume: u8) -> Result<()> {
        let mut sounds = self.active_sounds.lock().map_err(|_| {
            Error::AtmospherePlayback("Failed to acquire lock".to_string())
        })?;

        if let Some(active) = sounds.get_mut(url) {
            active.handle.set_volume(volume_to_db(volume), Tween::default());
            active.volume = volume;
            tracing::debug!("Set volume for {} to {}%", url, volume);
        }

        Ok(())
    }

    /// Gets the list of currently playing sound URLs.
    pub fn get_active_sounds(&self) -> Vec<String> {
        if let Ok(sounds) = self.active_sounds.lock() {
            sounds.keys().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Gets the number of pending downloads.
    pub fn pending_downloads(&self) -> usize {
        self.download_queue.pending_count()
    }

    /// Checks if a URL is currently being downloaded.
    pub fn is_downloading(&self, url: &str) -> bool {
        self.download_queue.is_downloading(url)
    }

    /// Gets all URLs currently being downloaded.
    pub fn get_downloading_urls(&self) -> Vec<String> {
        self.download_queue.get_downloading_urls()
    }

    /// Checks if a freesound URL is already cached locally.
    pub fn is_url_cached(&self, url: &str) -> bool {
        self.download_queue.find_cached_public(url).is_some()
    }

    /// Loads a sound manifest for resolving freesound URLs to local bundled files.
    pub fn load_manifest(&self, base_dir: &Path, manifest_path: &Path) {
        self.download_queue.load_manifest(base_dir, manifest_path);
    }

    /// Returns the number of entries in the sound manifest.
    pub fn manifest_size(&self) -> usize {
        self.download_queue.manifest_size()
    }

    /// Returns a snapshot of the current manifest (URL → absolute path).
    pub fn get_manifest(&self) -> HashMap<String, PathBuf> {
        self.download_queue.get_manifest()
    }

    /// Sets whether on-demand downloads are enabled.
    pub fn set_downloads_enabled(&self, enabled: bool) {
        self.download_queue.set_downloads_enabled(enabled);
    }

    /// Returns whether on-demand downloads are enabled.
    pub fn downloads_enabled(&self) -> bool {
        self.download_queue.downloads_enabled()
    }

    /// Enqueues a URL for download without starting playback.
    pub fn pre_download(&self, url: &str) -> bool {
        self.download_queue.enqueue(url, |_| {})
    }

    /// Pauses all currently playing sounds.
    pub fn pause_all(&self) {
        if let Ok(mut sounds) = self.active_sounds.lock() {
            for (url, active) in sounds.iter_mut() {
                active.handle.pause(Tween::default());
                tracing::debug!("Paused atmosphere sound: {}", url);
            }
        }
    }

    /// Resumes all currently paused sounds.
    pub fn resume_all(&self) {
        if let Ok(mut sounds) = self.active_sounds.lock() {
            for (url, active) in sounds.iter_mut() {
                active.handle.resume(Tween::default());
                tracing::debug!("Resumed atmosphere sound: {}", url);
            }
        }
    }

    /// Returns true if all active sounds are paused (or there are no active sounds).
    pub fn is_paused(&self) -> bool {
        if let Ok(sounds) = self.active_sounds.lock() {
            if sounds.is_empty() {
                return false;
            }
            sounds.values().all(|active| {
                matches!(active.handle.state(), PlaybackState::Paused | PlaybackState::Pausing)
            })
        } else {
            false
        }
    }

    /// Clears the download cache.
    pub fn clear_cache(&self) -> Result<usize> {
        let mut count = 0;

        if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                if std::fs::remove_file(entry.path()).is_ok() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

/// Internal function to start playback via the shared kira AudioManager.
///
/// Loads the file into a StaticSoundData, optionally sets it to loop, plays it
/// through the global AudioManager, and stores the handle for volume/stop control.
fn start_playback_internal(
    url: &str,
    file_path: &Path,
    volume: u8,
    active_sounds: &Arc<Mutex<HashMap<String, ActiveSound>>>,
    looping: bool,
    fade_duration: Option<u32>,
    max_duration: Option<u32>,
) -> Result<()> {
    // Load and configure sound
    let sound_data = match StaticSoundData::from_file(file_path) {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!(
                "Failed to decode {}, skipping: {}",
                file_path.display(),
                e
            );
            return Ok(());
        }
    };
    let sound_data = if looping {
        sound_data.loop_region(..).volume(volume_to_db(volume))
    } else {
        sound_data.volume(volume_to_db(volume))
    };

    // Play via shared AudioManager
    let handle = with_audio_manager(|mgr| mgr.play(sound_data))
        .ok_or_else(|| Error::AtmospherePlayback("No audio device available".into()))?
        .map_err(|e| Error::AtmospherePlayback(format!("{}", e)))?;

    // Store handle
    {
        let mut sounds = active_sounds.lock().map_err(|_| {
            Error::AtmospherePlayback("Failed to acquire lock".to_string())
        })?;
        sounds.insert(url.to_string(), ActiveSound { handle, volume });
    }

    tracing::info!(
        "Started atmosphere sound: {} at volume {}% (file: {})",
        url, volume, file_path.display()
    );

    // Handle max_duration and fade_duration with timer threads
    match (max_duration, fade_duration) {
        (Some(max_dur), Some(fade_dur)) => {
            // Both set: wait until (max_duration - fade_duration), then fade-stop
            let active_sounds = Arc::clone(active_sounds);
            let url_owned = url.to_string();
            std::thread::spawn(move || {
                let delay = if max_dur > fade_dur { max_dur - fade_dur } else { 0 };
                if delay > 0 {
                    std::thread::sleep(Duration::from_secs(delay as u64));
                }
                if let Ok(mut sounds) = active_sounds.lock() {
                    if let Some(mut active) = sounds.remove(&url_owned) {
                        active.handle.stop(Tween {
                            duration: Duration::from_secs(fade_dur as u64),
                            ..Default::default()
                        });
                        tracing::info!(
                            "Stopped atmosphere sound after {}s ({}s fade-out): {}",
                            max_dur, fade_dur, url_owned
                        );
                    }
                }
            });
        }
        (Some(duration), None) => {
            // Only max_duration: hard stop after N seconds
            let active_sounds = Arc::clone(active_sounds);
            let url_owned = url.to_string();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(duration as u64));
                if let Ok(mut sounds) = active_sounds.lock() {
                    if let Some(mut active) = sounds.remove(&url_owned) {
                        active.handle.stop(Tween::default());
                        tracing::info!("Stopped atmosphere sound after {}s max_duration: {}", duration, url_owned);
                    }
                }
            });
        }
        (None, Some(fade_dur)) => {
            // Only fade_duration: fade starts immediately, then clean up after fade completes
            // Start the fade immediately on the handle we just stored
            if let Ok(mut sounds) = active_sounds.lock() {
                if let Some(active) = sounds.get_mut(url) {
                    active.handle.stop(Tween {
                        duration: Duration::from_secs(fade_dur as u64),
                        ..Default::default()
                    });
                }
            }
            // Spawn a cleanup thread to remove from active_sounds after fade completes
            let active_sounds = Arc::clone(active_sounds);
            let url_owned = url.to_string();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(fade_dur as u64));
                if let Ok(mut sounds) = active_sounds.lock() {
                    sounds.remove(&url_owned);
                }
            });
        }
        (None, None) => {
            // No duration limits - sound loops until explicitly stopped
        }
    }

    Ok(())
}

/// Pool monitor loop — runs on a background thread, polls for track completion,
/// and starts the next random track from the pool.
fn pool_monitor_loop(
    pool_name: &str,
    mut current_url: String,
    start_generation: u64,
    active_sounds: &Arc<Mutex<HashMap<String, ActiveSound>>>,
    active_pools: &Arc<Mutex<HashMap<String, PoolConfig>>>,
    generation: &Arc<AtomicU64>,
    download_queue: &Arc<DownloadQueue>,
) {
    tracing::info!("Pool monitor started for '{}', playing: {}", pool_name, current_url);

    loop {
        std::thread::sleep(Duration::from_millis(1500));

        // Check generation — if changed, environment was switched
        if generation.load(Ordering::SeqCst) != start_generation {
            tracing::info!("Pool monitor '{}' exiting: generation changed", pool_name);
            return;
        }

        // Check if pool still exists
        let pool_info = {
            let pools = match active_pools.lock() {
                Ok(p) => p,
                Err(_) => return,
            };
            match pools.get(pool_name) {
                Some(pool) => Some((pool.urls.clone(), pool.volumes.clone())),
                None => None,
            }
        };
        let (urls, volumes) = match pool_info {
            Some(info) => info,
            None => {
                tracing::info!("Pool monitor '{}' exiting: pool removed", pool_name);
                return;
            }
        };

        // Check if current track is still playing
        let is_stopped = {
            let sounds = match active_sounds.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            match sounds.get(&current_url) {
                Some(active) => matches!(active.handle.state(), PlaybackState::Stopped),
                None => true, // Already removed
            }
        };

        if !is_stopped {
            continue;
        }

        // Current track finished — remove it from active_sounds
        if let Ok(mut sounds) = active_sounds.lock() {
            sounds.remove(&current_url);
        }
        tracing::info!("Pool '{}': track finished: {}", pool_name, current_url);

        // Pick next track (different from current if possible)
        let mut rng = rand::thread_rng();
        let next_url = if urls.len() > 1 {
            let other_urls: Vec<&String> = urls.iter().filter(|u| **u != current_url).collect();
            other_urls[rng.gen_range(0..other_urls.len())].clone()
        } else {
            urls[0].clone()
        };
        let volume = volumes.get(&next_url).copied().unwrap_or(70);

        // Start next track (non-looping)
        if let Some(cached_path) = download_queue.enqueue_or_get_cached(&next_url) {
            if let Err(e) = start_playback_internal(&next_url, &cached_path, volume, active_sounds, false, None, None) {
                tracing::warn!("Pool '{}': failed to start next track: {}", pool_name, e);
                return;
            }
        } else {
            tracing::warn!("Pool '{}': track not cached, skipping: {}", pool_name, next_url);
            // Try again next iteration with another track
            continue;
        }

        tracing::info!("Pool '{}': started next track: {} at volume {}%", pool_name, next_url, volume);
        current_url = next_url;
    }
}

impl Drop for AtmosphereEngine {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_dir_creation() {
        let temp_dir = TempDir::new().unwrap();
        let engine = AtmosphereEngine::new(temp_dir.path());

        assert!(engine.cache_dir.exists());
    }

    #[test]
    fn test_get_active_sounds_empty() {
        let temp_dir = TempDir::new().unwrap();
        let engine = AtmosphereEngine::new(temp_dir.path());

        let active = engine.get_active_sounds();
        assert!(active.is_empty());
    }

    #[test]
    fn test_stop_all_on_empty() {
        let temp_dir = TempDir::new().unwrap();
        let engine = AtmosphereEngine::new(temp_dir.path());

        // Should not panic when stopping with no active sounds
        engine.stop_all();
        assert!(engine.get_active_sounds().is_empty());
    }

    #[test]
    fn test_active_sounds_consistency_after_stop() {
        let temp_dir = TempDir::new().unwrap();
        let engine = AtmosphereEngine::new(temp_dir.path());

        // Stop all, then check active sounds is still consistent
        engine.stop_all();
        let active = engine.get_active_sounds();
        assert_eq!(active.len(), 0, "Active sounds should be empty after stop_all");
    }

    #[test]
    fn test_stop_all_except() {
        let temp_dir = TempDir::new().unwrap();
        let engine = AtmosphereEngine::new(temp_dir.path());

        // On empty, stop_all_except should return 0
        let mut keep = std::collections::HashSet::new();
        keep.insert("http://test/a".to_string());
        let stopped = engine.stop_all_except(&keep);
        assert_eq!(stopped, 0);
    }

    /// Test that multiple atmosphere sounds survive playback for several seconds
    /// using kira's internal mixer.
    ///
    /// Requires a working audio device — skip in CI.
    #[test]
    #[ignore] // Requires audio device — run with: cargo test -- --ignored
    fn test_multiple_atmosphere_sounds_survive_playback() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("freesound.org");
        std::fs::create_dir_all(&cache_dir).unwrap();

        // Create test WAV files with different frequencies
        create_test_wav(&cache_dir.join("sound_a.wav"), 440.0, 1.0, 44100);
        create_test_wav(&cache_dir.join("sound_b.wav"), 550.0, 1.0, 44100);
        create_test_wav(&cache_dir.join("sound_c.wav"), 660.0, 1.0, 44100);

        let active_sounds: Arc<Mutex<HashMap<String, ActiveSound>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Start 3 sounds via kira's shared AudioManager
        start_playback_internal(
            "http://test/a",
            &cache_dir.join("sound_a.wav"),
            70,
            &active_sounds,
            true,
            None,
            None,
        )
        .unwrap();

        start_playback_internal(
            "http://test/b",
            &cache_dir.join("sound_b.wav"),
            50,
            &active_sounds,
            true,
            None,
            None,
        )
        .unwrap();

        start_playback_internal(
            "http://test/c",
            &cache_dir.join("sound_c.wav"),
            30,
            &active_sounds,
            true,
            None,
            None,
        )
        .unwrap();

        // THE CRITICAL CHECK: wait long enough for the old bug to manifest.
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Verify all 3 are STILL tracked AND their handles are still playing
        {
            let sounds = active_sounds.lock().unwrap();
            assert_eq!(sounds.len(), 3, "Expected 3 active sounds after 2s");
            for (url, active) in sounds.iter() {
                assert!(
                    matches!(active.handle.state(), PlaybackState::Playing),
                    "Handle for {} should still be Playing, got {:?}",
                    url,
                    active.handle.state()
                );
            }
        }

        // Stop one, wait, verify remaining are still alive
        {
            let mut sounds = active_sounds.lock().unwrap();
            if let Some(mut active) = sounds.remove("http://test/b") {
                active.handle.stop(Tween::default());
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        {
            let sounds = active_sounds.lock().unwrap();
            assert_eq!(
                sounds.len(),
                2,
                "Expected 2 active sounds after stopping one"
            );
            assert!(!sounds.contains_key("http://test/b"));
            for (url, active) in sounds.iter() {
                assert!(
                    matches!(active.handle.state(), PlaybackState::Playing),
                    "Handle for {} died after stopping a different sound",
                    url
                );
            }
        }

        // Stop all remaining
        {
            let mut sounds = active_sounds.lock().unwrap();
            for (_url, mut active) in sounds.drain() {
                active.handle.stop(Tween::default());
            }
        }

        {
            let sounds = active_sounds.lock().unwrap();
            assert_eq!(sounds.len(), 0, "Expected 0 active sounds after stop all");
        }
    }

    /// Helper: create a WAV file with a sine wave for testing.
    #[allow(dead_code)]
    fn create_test_wav(path: &Path, frequency: f32, duration_secs: f32, sample_rate: u32) {
        use hound;

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = (t * frequency * 2.0 * std::f32::consts::PI).sin();
            writer
                .write_sample((sample * i16::MAX as f32) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
    }
}
