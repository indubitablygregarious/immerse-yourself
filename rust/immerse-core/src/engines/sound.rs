//! Sound engine for playing audio files via kira.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::PlaybackState;
use kira::Tween;

use crate::download_queue::{download_sound, find_downloaded_file, parse_freesound_url};
use crate::engines::audio_output::{is_audio_available, volume_to_db, with_audio_manager};
use crate::error::{Error, Result};

/// Sound engine that plays audio files using kira.
pub struct SoundEngine {
    project_root: PathBuf,
    cache_dir: PathBuf,
    user_content_dir: Option<PathBuf>,
    active_handles: Arc<Mutex<Vec<StaticSoundHandle>>>,
    available: bool,
}

impl SoundEngine {
    /// Creates a new sound engine with cache dir at `project_root/freesound.org/`.
    pub fn new<P: AsRef<Path>>(project_root: P) -> Self {
        let root = project_root.as_ref().to_path_buf();
        let cache_dir = root.join("freesound.org");
        Self::new_with_cache_dir(root, cache_dir)
    }

    /// Creates a new sound engine with an explicit cache directory for freesound downloads.
    pub fn new_with_cache_dir(project_root: PathBuf, cache_dir: PathBuf) -> Self {
        let available = is_audio_available();
        if !available {
            tracing::warn!("No audio output device detected. Sound playback will be disabled.");
        }

        Self {
            project_root,
            cache_dir,
            user_content_dir: None,
            active_handles: Arc::new(Mutex::new(Vec::new())),
            available,
        }
    }

    /// Sets the user content directory for fallback sound resolution.
    pub fn set_user_content_dir(&mut self, dir: PathBuf) {
        self.user_content_dir = Some(dir);
    }

    /// Plays a sound file synchronously (blocks until complete).
    pub fn play(&self, file: &str) -> Result<()> {
        let resolved = self.resolve_sound(file)?;
        let sound_data = StaticSoundData::from_file(&resolved.path)
            .map_err(|e| Error::SoundPlayback(format!("Failed to load {}: {}", resolved.path.display(), e)))?;

        let mut handle = with_audio_manager(|mgr| mgr.play(sound_data))
            .ok_or(Error::NoAudioPlayer)?
            .map_err(|e| Error::SoundPlayback(format!("{}", e)))?;

        // Block until sound finishes
        loop {
            match handle.state() {
                PlaybackState::Playing | PlaybackState::Pausing | PlaybackState::Resuming => {}
                _ => break,
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        handle.stop(Tween::default());
        Ok(())
    }

    /// Plays a sound file asynchronously, returning immediately.
    pub fn play_async(&self, file: &str) -> Result<()> {
        self.play_async_with_volume(file, 100)
    }

    /// Plays a sound file asynchronously with a specific volume (0-100).
    /// If the sound comes from a sound_conf with max_duration/fadeout, those are applied.
    pub fn play_async_with_volume(&self, file: &str, volume: u8) -> Result<()> {
        let resolved = self.resolve_sound(file)?;
        let sound_data = StaticSoundData::from_file(&resolved.path)
            .map_err(|e| Error::SoundPlayback(format!("Failed to load {}: {}", resolved.path.display(), e)))?
            .volume(volume_to_db(volume));

        let mut handle = with_audio_manager(|mgr| mgr.play(sound_data))
            .ok_or(Error::NoAudioPlayer)?
            .map_err(|e| Error::SoundPlayback(format!("{}", e)))?;

        // Apply fadeout if configured (from sound_conf YAML)
        if let Some(fadeout_ms) = resolved.fadeout {
            let wait_ms = if let Some(max_dur) = resolved.max_duration {
                // Wait until (max_duration - fadeout) before starting fade
                max_dur.saturating_sub(fadeout_ms) as u64
            } else {
                // No max_duration: start fading immediately
                0
            };

            std::thread::spawn(move || {
                if wait_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(wait_ms));
                }
                handle.stop(Tween {
                    duration: std::time::Duration::from_millis(fadeout_ms as u64),
                    ..Default::default()
                });
            });
        } else {
            if let Ok(mut handles) = self.active_handles.lock() {
                handles.push(handle);
            }
        }

        Ok(())
    }

    /// Plays a sound file asynchronously with a completion callback.
    pub fn play_async_with_callback<F>(&self, file: &str, on_complete: F) -> Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        let resolved = self.resolve_sound(file)?;
        let sound_data = StaticSoundData::from_file(&resolved.path)
            .map_err(|e| Error::SoundPlayback(format!("Failed to load {}: {}", resolved.path.display(), e)))?;

        let handle = with_audio_manager(|mgr| mgr.play(sound_data))
            .ok_or(Error::NoAudioPlayer)?
            .map_err(|e| Error::SoundPlayback(format!("{}", e)))?;

        if let Ok(mut handles) = self.active_handles.lock() {
            handles.push(handle);
        }

        let handles_clone = Arc::clone(&self.active_handles);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
                let done = if let Ok(handles) = handles_clone.lock() {
                    handles.last().map_or(true, |h| {
                        !matches!(h.state(), PlaybackState::Playing | PlaybackState::Pausing | PlaybackState::Resuming)
                    })
                } else {
                    true
                };
                if done {
                    break;
                }
            }
            on_complete();
        });

        Ok(())
    }

    /// Stops all currently playing sounds.
    pub fn stop_all(&self) -> usize {
        let mut count = 0;

        if let Ok(mut handles) = self.active_handles.lock() {
            for mut handle in handles.drain(..) {
                handle.stop(Tween::default());
                count += 1;
            }
        }

        count
    }

    /// Pauses all currently playing sounds.
    pub fn pause_all(&self) {
        if let Ok(mut handles) = self.active_handles.lock() {
            for handle in handles.iter_mut() {
                handle.pause(Tween::default());
            }
        }
    }

    /// Resumes all currently paused sounds.
    pub fn resume_all(&self) {
        if let Ok(mut handles) = self.active_handles.lock() {
            for handle in handles.iter_mut() {
                handle.resume(Tween::default());
            }
        }
    }

    /// Resolves a file reference to a ResolvedSound with path and optional playback metadata.
    /// For sound_conf references, metadata comes from the YAML config.
    /// For plain file paths, metadata is None.
    fn resolve_sound(&self, file: &str) -> Result<ResolvedSound> {
        if file.starts_with("sound_conf:") {
            return self.resolve_sound_conf(file);
        }
        Ok(ResolvedSound {
            path: self.resolve_path(file)?,
            max_duration: None,
            fadeout: None,
        })
    }

    /// Resolves a file path, handling both absolute and relative paths.
    /// Search order: absolute → project_root → project_root/sounds →
    /// user_content_dir → user_content_dir/sounds → error
    fn resolve_path(&self, file: &str) -> Result<PathBuf> {
        let path = if Path::new(file).is_absolute() {
            PathBuf::from(file)
        } else {
            // Try relative to project root
            let rel_path = self.project_root.join(file);
            if rel_path.exists() {
                rel_path
            } else {
                // Try in sounds directory
                let sounds_path = self.project_root.join("sounds").join(file);
                if sounds_path.exists() {
                    sounds_path
                } else if let Some(ref user_dir) = self.user_content_dir {
                    // Try in user content directory
                    let user_path = user_dir.join(file);
                    if user_path.exists() {
                        user_path
                    } else {
                        // Try in user content sounds directory
                        let user_sounds_path = user_dir.join("sounds").join(file);
                        if user_sounds_path.exists() {
                            user_sounds_path
                        } else {
                            return Err(Error::SoundFileNotFound(file.to_string()));
                        }
                    }
                } else {
                    return Err(Error::SoundFileNotFound(file.to_string()));
                }
            }
        };

        if !path.exists() {
            return Err(Error::SoundFileNotFound(path.display().to_string()));
        }

        Ok(path)
    }

    /// Resolves a sound_conf reference to a random sound from the collection.
    /// Searches project_root/sound_conf/ first, then user_content_dir/sound_conf/.
    fn resolve_sound_conf(&self, reference: &str) -> Result<ResolvedSound> {
        let conf_name = reference.strip_prefix("sound_conf:").unwrap();
        let conf_path = self
            .project_root
            .join("sound_conf")
            .join(format!("{}.yaml", conf_name));

        // Fall back to user content directory if not found in project root
        let conf_path = if conf_path.exists() {
            conf_path
        } else if let Some(ref user_dir) = self.user_content_dir {
            let user_conf_path = user_dir
                .join("sound_conf")
                .join(format!("{}.yaml", conf_name));
            if user_conf_path.exists() {
                user_conf_path
            } else {
                return Err(Error::SoundConfNotFound(conf_name.to_string()));
            }
        } else {
            return Err(Error::SoundConfNotFound(conf_name.to_string()));
        };

        // Load the sound_conf YAML
        let content = std::fs::read_to_string(&conf_path)
            .map_err(|e| Error::ConfigLoad(conf_path.display().to_string(), e.to_string()))?;

        let conf: SoundConfConfig = serde_yaml::from_str(&content)
            .map_err(|e| Error::ConfigParse(conf_path.display().to_string(), e.to_string()))?;

        if conf.sounds.is_empty() {
            return Err(Error::SoundConfNotFound(format!(
                "{} has no sounds",
                conf_name
            )));
        }

        // Pick a random sound
        use rand::seq::SliceRandom;
        let sound = conf.sounds.choose(&mut rand::thread_rng()).unwrap();

        // Per-sound values override collection-level values
        let max_duration = sound.max_duration.or(conf.max_duration);
        let fadeout = sound.fadeout.or(conf.fadeout);

        // Handle local file vs URL
        let path = if let Some(ref file) = sound.file {
            self.resolve_path(file)?
        } else if let Some(ref url) = sound.url {
            // Download the URL to cache if needed
            self.download_sound_url(url)?
        } else {
            return Err(Error::SoundConfNotFound(format!(
                "Sound in {} has neither file nor url",
                conf_name
            )));
        };

        Ok(ResolvedSound {
            path,
            max_duration,
            fadeout,
        })
    }

    /// Downloads a sound from a URL to the cache directory.
    /// Runs the download on a separate OS thread to avoid panicking when called
    /// from within Tauri's Tokio runtime (reqwest::blocking creates its own runtime).
    fn download_sound_url(&self, url: &str) -> Result<PathBuf> {
        let cache_dir = &self.cache_dir;
        std::fs::create_dir_all(cache_dir).ok();

        // Check if already downloaded (using the standard naming convention)
        if let Some((creator, sound_id)) = parse_freesound_url(url) {
            if let Some(cached) = find_downloaded_file(cache_dir, &creator, &sound_id) {
                tracing::info!("Sound already cached: {:?}", cached);
                return Ok(cached);
            }
        }

        // Must run on a plain OS thread — reqwest::blocking creates an internal
        // Tokio runtime which panics if nested inside Tauri's runtime.
        let cache_dir_owned = cache_dir.clone();
        let url_owned = url.to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(download_sound(&url_owned, &cache_dir_owned));
        });

        rx.recv()
            .map_err(|_| Error::SoundPlayback("Download thread failed".to_string()))?
            .map_err(Error::SoundPlayback)
    }

    /// Returns the number of currently playing sounds.
    /// Cleans up finished handles as a side effect.
    pub fn playing_count(&self) -> usize {
        if let Ok(mut handles) = self.active_handles.lock() {
            handles.retain(|h| matches!(h.state(), PlaybackState::Playing | PlaybackState::Pausing | PlaybackState::Resuming));
            handles.len()
        } else {
            0
        }
    }

    /// Returns whether an audio output device is available.
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Returns the name of the audio backend.
    pub fn player_name(&self) -> Option<&str> {
        if self.available {
            Some("kira")
        } else {
            None
        }
    }
}

/// Sound configuration file format.
#[derive(Debug, serde::Deserialize)]
struct SoundConfConfig {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    /// Collection-level max duration in ms (can be overridden per-sound).
    max_duration: Option<u32>,
    /// Collection-level fadeout duration in ms (can be overridden per-sound).
    fadeout: Option<u32>,
    sounds: Vec<SoundEntry>,
}

/// Individual sound entry in a sound_conf.
#[derive(Debug, serde::Deserialize)]
struct SoundEntry {
    file: Option<String>,
    url: Option<String>,
    /// Per-sound max duration in ms (overrides collection-level).
    max_duration: Option<u32>,
    /// Per-sound fadeout duration in ms (overrides collection-level).
    fadeout: Option<u32>,
}

/// A resolved sound with its file path and optional playback metadata.
struct ResolvedSound {
    path: PathBuf,
    /// Max total playback duration in ms (including fade).
    max_duration: Option<u32>,
    /// Fadeout duration in ms.
    fadeout: Option<u32>,
}

impl Drop for SoundEngine {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sound_engine_creation() {
        let temp_dir = TempDir::new().unwrap();
        let engine = SoundEngine::new(temp_dir.path());
        // Engine should be created regardless of audio device availability
        assert!(engine.project_root.exists());
    }

    #[test]
    fn test_resolve_path_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let engine = SoundEngine::new(temp_dir.path());

        let result = engine.resolve_path("nonexistent.wav");
        assert!(matches!(result, Err(Error::SoundFileNotFound(_))));
    }

    /// Verifies that download_sound_url runs on a separate OS thread and doesn't
    /// panic with "Cannot drop a runtime" when called inside an existing Tokio runtime.
    #[test]
    fn test_download_sound_url_no_nested_runtime_panic() {
        // Create a Tokio runtime to simulate being called from within Tauri
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let temp_dir = TempDir::new().unwrap();
            let engine = SoundEngine::new(temp_dir.path());

            // This URL won't resolve (no network in tests), but the important thing
            // is it doesn't panic with a nested runtime error. It should return an error.
            let result = engine.download_sound_url("https://freesound.org/people/testuser/sounds/99999/");
            assert!(result.is_err(), "Expected download error (no network), but should not panic");
        });
    }

    #[test]
    fn test_download_sound_url_returns_cached_file() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("freesound.org");
        std::fs::create_dir_all(&cache_dir).unwrap();

        // Create a fake cached file matching the freesound naming convention:
        // find_downloaded_file expects "{creator}_{sound_id}_" prefix
        let cached_file = cache_dir.join("testuser_12345_campfire.wav");
        std::fs::write(&cached_file, b"fake audio data").unwrap();

        let engine = SoundEngine::new(temp_dir.path());

        let result = engine.download_sound_url("https://freesound.org/people/testuser/sounds/12345/");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), cached_file);
    }

    #[test]
    fn test_stop_all_returns_count() {
        let temp_dir = TempDir::new().unwrap();
        let engine = SoundEngine::new(temp_dir.path());

        // No handles active, should return 0
        assert_eq!(engine.stop_all(), 0);
    }

    #[test]
    fn test_playing_count_empty() {
        let temp_dir = TempDir::new().unwrap();
        let engine = SoundEngine::new(temp_dir.path());

        assert_eq!(engine.playing_count(), 0);
    }
}
