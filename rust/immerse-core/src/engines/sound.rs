//! Sound engine for playing audio files via rodio.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rodio::{Decoder, Sink};

use crate::download_queue::{download_sound, find_downloaded_file, parse_freesound_url};
use crate::engines::audio_output::get_output_stream_handle;
use crate::error::{Error, Result};

/// Sound engine that plays audio files using rodio.
pub struct SoundEngine {
    project_root: PathBuf,
    cache_dir: PathBuf,
    user_content_dir: Option<PathBuf>,
    active_sinks: Arc<Mutex<Vec<Sink>>>,
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
        let available = get_output_stream_handle().is_some();
        if !available {
            tracing::warn!("No audio output device detected. Sound playback will be disabled.");
        }

        Self {
            project_root,
            cache_dir,
            user_content_dir: None,
            active_sinks: Arc::new(Mutex::new(Vec::new())),
            available,
        }
    }

    /// Sets the user content directory for fallback sound resolution.
    pub fn set_user_content_dir(&mut self, dir: PathBuf) {
        self.user_content_dir = Some(dir);
    }

    /// Plays a sound file synchronously (blocks until complete).
    pub fn play(&self, file: &str) -> Result<()> {
        let handle = get_output_stream_handle().ok_or(Error::NoAudioPlayer)?;
        let path = self.resolve_path(file)?;

        let file = std::fs::File::open(&path)
            .map_err(|e| Error::SoundPlayback(format!("Failed to open {}: {}", path.display(), e)))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| Error::SoundPlayback(format!("Failed to decode {}: {}", path.display(), e)))?;

        let sink = Sink::try_new(handle)
            .map_err(|e| Error::SoundPlayback(e.to_string()))?;
        sink.append(source);
        sink.sleep_until_end();

        Ok(())
    }

    /// Plays a sound file asynchronously, returning immediately.
    pub fn play_async(&self, file: &str) -> Result<()> {
        self.play_async_with_volume(file, 100)
    }

    /// Plays a sound file asynchronously with a specific volume (0-100).
    pub fn play_async_with_volume(&self, file: &str, volume: u8) -> Result<()> {
        let handle = get_output_stream_handle().ok_or(Error::NoAudioPlayer)?;
        let path = self.resolve_path(file)?;

        let file = std::fs::File::open(&path)
            .map_err(|e| Error::SoundPlayback(format!("Failed to open {}: {}", path.display(), e)))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| Error::SoundPlayback(format!("Failed to decode {}: {}", path.display(), e)))?;

        let sink = Sink::try_new(handle)
            .map_err(|e| Error::SoundPlayback(e.to_string()))?;
        sink.set_volume(volume as f32 / 100.0);
        sink.append(source);

        if let Ok(mut sinks) = self.active_sinks.lock() {
            sinks.push(sink);
        }

        Ok(())
    }

    /// Plays a sound file asynchronously with a completion callback.
    pub fn play_async_with_callback<F>(&self, file: &str, on_complete: F) -> Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = get_output_stream_handle().ok_or(Error::NoAudioPlayer)?;
        let path = self.resolve_path(file)?;

        let file = std::fs::File::open(&path)
            .map_err(|e| Error::SoundPlayback(format!("Failed to open {}: {}", path.display(), e)))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| Error::SoundPlayback(format!("Failed to decode {}: {}", path.display(), e)))?;

        let sink = Sink::try_new(handle)
            .map_err(|e| Error::SoundPlayback(e.to_string()))?;
        sink.append(source);

        let sinks = Arc::clone(&self.active_sinks);
        if let Ok(mut s) = sinks.lock() {
            s.push(sink);
        }

        // Spawn thread that waits for the last-added sink to finish, then calls callback.
        // We find it by checking empty state on the sink we just added (last in vec).
        std::thread::spawn(move || {
            // Poll until our sink is done. We can't easily hold the Sink outside the vec,
            // so just wait in a loop checking the last sink.
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
                let done = if let Ok(s) = sinks.lock() {
                    // Check if all sinks are done (conservative approach)
                    s.last().map_or(true, |sink| sink.empty())
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

        if let Ok(mut sinks) = self.active_sinks.lock() {
            for sink in sinks.drain(..) {
                sink.stop();
                count += 1;
            }
        }

        count
    }

    /// Pauses all currently playing sounds.
    pub fn pause_all(&self) {
        if let Ok(sinks) = self.active_sinks.lock() {
            for sink in sinks.iter() {
                sink.pause();
            }
        }
    }

    /// Resumes all currently paused sounds.
    pub fn resume_all(&self) {
        if let Ok(sinks) = self.active_sinks.lock() {
            for sink in sinks.iter() {
                sink.play();
            }
        }
    }

    /// Resolves a file path, handling both absolute and relative paths.
    /// Search order: sound_conf → absolute → project_root → project_root/sounds →
    /// user_content_dir → user_content_dir/sounds → error
    fn resolve_path(&self, file: &str) -> Result<PathBuf> {
        // Handle sound_conf references
        if file.starts_with("sound_conf:") {
            return self.resolve_sound_conf(file);
        }

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
    fn resolve_sound_conf(&self, reference: &str) -> Result<PathBuf> {
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

        // Handle local file vs URL
        if let Some(ref file) = sound.file {
            self.resolve_path(file)
        } else if let Some(ref url) = sound.url {
            // Download the URL to cache if needed
            self.download_sound_url(url)
        } else {
            Err(Error::SoundConfNotFound(format!(
                "Sound in {} has neither file nor url",
                conf_name
            )))
        }
    }

    /// Downloads a sound from a URL to the cache directory.
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

        // Download using the shared download function (same as atmosphere engine)
        download_sound(url, cache_dir).map_err(|e| Error::SoundPlayback(e))
    }

    /// Returns the number of currently playing sounds.
    /// Cleans up finished sinks as a side effect.
    pub fn playing_count(&self) -> usize {
        if let Ok(mut sinks) = self.active_sinks.lock() {
            sinks.retain(|sink| !sink.empty());
            sinks.len()
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
            Some("rodio")
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
    sounds: Vec<SoundEntry>,
}

/// Individual sound entry in a sound_conf.
#[derive(Debug, serde::Deserialize)]
struct SoundEntry {
    file: Option<String>,
    url: Option<String>,
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
}
