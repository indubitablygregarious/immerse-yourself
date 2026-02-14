//! Shared audio output singleton.
//!
//! kira's `AudioManager` owns a single cpal stream internally and runs a real
//! mixer on its own audio thread. Unlike rodio's `OutputStream`, `AudioManager`
//! is `Send`, so no dedicated background thread is needed.

use std::sync::{Mutex, OnceLock};

use kira::{AudioManager, AudioManagerSettings, DefaultBackend, Decibels};

/// Global audio manager singleton. kira's AudioManager owns the single cpal
/// stream internally — all sounds play through its mixer.
static AUDIO_MANAGER: OnceLock<Option<Mutex<AudioManager<DefaultBackend>>>> = OnceLock::new();

/// Executes a closure with mutable access to the shared AudioManager.
/// Returns None if no audio device is available.
pub fn with_audio_manager<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AudioManager<DefaultBackend>) -> R,
{
    let slot = AUDIO_MANAGER.get_or_init(|| {
        match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(manager) => Some(Mutex::new(manager)),
            Err(e) => {
                tracing::error!("Failed to initialize audio: {}", e);
                None
            }
        }
    });
    slot.as_ref().map(|m| f(&mut m.lock().unwrap()))
}

/// Returns whether an audio device is available.
pub fn is_audio_available() -> bool {
    with_audio_manager(|_| ()).is_some()
}

/// Converts 0-100 integer volume to kira Decibels.
pub fn volume_to_db(volume: u8) -> Decibels {
    if volume == 0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * (volume as f32 / 100.0).log10())
    }
}
