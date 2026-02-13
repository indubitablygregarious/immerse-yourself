//! Atmosphere engine for ambient sound playback from freesound.org URLs.

use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rodio::{Decoder, Sink, Source};

use crate::download_queue::DownloadQueue;
use crate::engines::audio_output::get_output_stream_handle;
use crate::error::{Error, Result};

/// Atmosphere engine for playing looping ambient sounds.
pub struct AtmosphereEngine {
    cache_dir: PathBuf,
    active_sounds: Arc<Mutex<HashMap<String, ActiveSound>>>,
    download_queue: Arc<DownloadQueue>,
    /// Generation counter - incremented on stop_all() to invalidate pending download callbacks.
    generation: Arc<AtomicU64>,
}

/// A playing atmosphere sound with its rodio Sink.
struct ActiveSound {
    sink: Arc<Sink>,
    #[allow(dead_code)]
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
            return start_playback_internal(&url_owned, &cached_path, volume, &active_sounds, fade_duration, max_duration);
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
                    if let Err(e) = start_playback_internal(&url_owned, &path, volume_copy, &active_sounds, fade_duration, max_duration) {
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

    /// Stops a single sound.
    pub fn stop_single(&self, url: &str) -> Result<()> {
        let mut sounds = self.active_sounds.lock().map_err(|_| {
            Error::AtmospherePlayback("Failed to acquire lock".to_string())
        })?;

        if let Some(active) = sounds.remove(url) {
            active.sink.stop();
            tracing::info!("Stopped atmosphere sound: {}", url);
        }

        Ok(())
    }

    /// Stops all playing sounds and invalidates pending download callbacks.
    pub fn stop_all(&self) -> usize {
        let old_gen = self.generation.fetch_add(1, Ordering::SeqCst);
        tracing::info!("stop_all: incremented generation from {} to {}", old_gen, old_gen + 1);

        let mut count = 0;

        if let Ok(mut sounds) = self.active_sounds.lock() {
            for (url, active) in sounds.drain() {
                active.sink.stop();
                count += 1;
                tracing::info!("Stopped atmosphere sound: {}", url);
            }
        }

        count
    }

    /// Sets the volume for a playing sound.
    pub fn set_volume(&self, url: &str, volume: u8) -> Result<()> {
        let sounds = self.active_sounds.lock().map_err(|_| {
            Error::AtmospherePlayback("Failed to acquire lock".to_string())
        })?;

        if let Some(active) = sounds.get(url) {
            active.sink.set_volume(volume as f32 / 100.0);
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

    /// Enqueues a URL for download without starting playback.
    pub fn pre_download(&self, url: &str) -> bool {
        self.download_queue.enqueue(url, |_| {})
    }

    /// Pauses all currently playing sounds.
    pub fn pause_all(&self) {
        if let Ok(sounds) = self.active_sounds.lock() {
            for (url, active) in sounds.iter() {
                active.sink.pause();
                tracing::debug!("Paused atmosphere sound: {}", url);
            }
        }
    }

    /// Resumes all currently paused sounds.
    pub fn resume_all(&self) {
        if let Ok(sounds) = self.active_sounds.lock() {
            for (url, active) in sounds.iter() {
                active.sink.play();
                tracing::debug!("Resumed atmosphere sound: {}", url);
            }
        }
    }

    /// Returns true if all active sinks are paused (or there are no active sounds).
    pub fn is_paused(&self) -> bool {
        if let Ok(sounds) = self.active_sounds.lock() {
            if sounds.is_empty() {
                return false;
            }
            sounds.values().all(|active| active.sink.is_paused())
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

/// Internal function to start playback (used both directly and from callback).
fn start_playback_internal(
    url: &str,
    file_path: &Path,
    volume: u8,
    active_sounds: &Arc<Mutex<HashMap<String, ActiveSound>>>,
    fade_duration: Option<u32>,
    max_duration: Option<u32>,
) -> Result<()> {
    let handle = get_output_stream_handle().ok_or_else(|| {
        Error::AtmospherePlayback("No audio output device available".to_string())
    })?;

    let file = std::fs::File::open(file_path)
        .map_err(|e| Error::AtmospherePlayback(format!("Failed to open {}: {}", file_path.display(), e)))?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| Error::AtmospherePlayback(format!("Failed to decode {}: {}", file_path.display(), e)))?;

    // Loop the source infinitely (replaces ffplay -loop 0)
    let looping_source = source.repeat_infinite();

    let sink = Sink::try_new(handle)
        .map_err(|e| Error::AtmospherePlayback(e.to_string()))?;
    sink.set_volume(volume as f32 / 100.0);
    sink.append(looping_source);

    let sink = Arc::new(sink);

    // Track the sound
    let url_owned = url.to_string();
    {
        let mut sounds = active_sounds.lock().map_err(|_| {
            Error::AtmospherePlayback("Failed to acquire lock".to_string())
        })?;
        sounds.insert(url_owned.clone(), ActiveSound { sink: Arc::clone(&sink), volume });
    }

    tracing::info!("Started atmosphere sound: {} at volume {}%", url, volume);

    // Handle max_duration and fade_duration
    match (max_duration, fade_duration) {
        (Some(max_dur), Some(fade_dur)) => {
            // Both set: wait until (max_duration - fade_duration), then fade out
            let active_sounds = Arc::clone(active_sounds);
            let fade_sink = Arc::clone(&sink);
            let initial_volume = volume;
            std::thread::spawn(move || {
                let delay_before_fade = if max_dur > fade_dur { max_dur - fade_dur } else { 0 };
                if delay_before_fade > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(delay_before_fade as u64));
                }

                // Fade out over fade_dur seconds
                let steps = 20u32;
                let step_duration_ms = (fade_dur as u64 * 1000) / steps as u64;

                for step in 1..=steps {
                    std::thread::sleep(std::time::Duration::from_millis(step_duration_ms));

                    // Check if sound is still in active_sounds (may have been stopped externally)
                    {
                        let sounds = match active_sounds.lock() {
                            Ok(s) => s,
                            Err(_) => return,
                        };
                        if !sounds.contains_key(&url_owned) {
                            tracing::debug!("Fade aborted for {} - sound no longer active", url_owned);
                            return;
                        }
                    }

                    let progress = step as f32 / steps as f32;
                    let new_volume = ((1.0 - progress) * initial_volume as f32).max(5.0) / 100.0;
                    fade_sink.set_volume(new_volume);
                }

                // Stop the sound after fade completes
                if let Ok(mut sounds) = active_sounds.lock() {
                    if sounds.remove(&url_owned).is_some() {
                        fade_sink.stop();
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
            let stop_sink = Arc::clone(&sink);
            let url_for_max = url_owned.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(duration as u64));

                if let Ok(mut sounds) = active_sounds.lock() {
                    if sounds.remove(&url_for_max).is_some() {
                        stop_sink.stop();
                        tracing::info!("Stopped atmosphere sound after {}s max_duration: {}", duration, url_for_max);
                    }
                }
            });
        }
        (None, Some(duration)) => {
            // Only fade_duration: fade starts immediately
            let active_sounds = Arc::clone(active_sounds);
            let fade_sink = Arc::clone(&sink);
            let initial_volume = volume;
            std::thread::spawn(move || {
                let steps = 20u32;
                let step_duration_ms = (duration as u64 * 1000) / steps as u64;

                for step in 1..=steps {
                    std::thread::sleep(std::time::Duration::from_millis(step_duration_ms));

                    {
                        let sounds = match active_sounds.lock() {
                            Ok(s) => s,
                            Err(_) => return,
                        };
                        if !sounds.contains_key(&url_owned) {
                            tracing::debug!("Fade aborted for {} - sound no longer active", url_owned);
                            return;
                        }
                    }

                    let progress = step as f32 / steps as f32;
                    let new_volume = ((1.0 - progress) * initial_volume as f32).max(5.0) / 100.0;
                    fade_sink.set_volume(new_volume);
                }

                // Stop the sound after fade completes
                if let Ok(mut sounds) = active_sounds.lock() {
                    if sounds.remove(&url_owned).is_some() {
                        fade_sink.stop();
                        tracing::info!("Stopped atmosphere sound after {}s fade-out: {}", duration, url_owned);
                    }
                }
            });
        }
        (None, None) => {
            // No duration limits - sound loops until explicitly stopped
        }
    }

    Ok(())
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
}
