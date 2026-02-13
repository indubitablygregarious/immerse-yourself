//! Immerse Core - Core library for the Immerse Yourself ambient environment system.
//!
//! This library provides:
//! - Configuration loading and validation for environment YAML files
//! - Sound engine for playing audio files
//! - Spotify engine for music playback control
//! - Lights engine for WIZ smart bulb control
//! - Atmosphere engine for ambient sound loops
//! - FFI layer for Swift/iOS interop
//!
//! # Example
//!
//! ```rust,no_run
//! use immerse_core::config::ConfigLoader;
//! use immerse_core::engines::SoundEngine;
//!
//! // Load environment configs
//! let loader = ConfigLoader::new("env_conf");
//! let configs = loader.load_all().unwrap();
//!
//! // Play a sound
//! let sound = SoundEngine::new(".");
//! sound.play_async("sounds/whoosh.wav").unwrap();
//! ```

pub mod config;
pub mod download_queue;
pub mod engines;
pub mod error;
pub mod ffi;

pub use error::{Error, Result};

/// Re-export commonly used types.
pub mod prelude {
    pub use crate::config::{
        AnimationConfig, AtmosphereConfig, ConfigLoader, EnvironmentConfig,
        LightGroupConfig, LightsConfig, Metadata, SoundConfig, SoundMix,
        SpotifyConfig, TimeOfDay,
    };
    pub use crate::engines::{
        AtmosphereEngine, LightsEngine, SoundEngine, SpotifyCredentials,
        SpotifyDevice, SpotifyEngine,
    };
    pub use crate::error::{Error, Result};
}
