//! Engine implementations for sound, lights, Spotify, and atmosphere.

pub mod audio_output;
mod atmosphere;
mod lights;
mod sound;
mod spotify;

pub use atmosphere::AtmosphereEngine;
pub use lights::LightsEngine;
pub use sound::SoundEngine;
pub use spotify::{
    is_spotify_in_path, is_spotify_running, start_spotify, SpotifyCredentials,
    SpotifyDevice, SpotifyEngine,
};
