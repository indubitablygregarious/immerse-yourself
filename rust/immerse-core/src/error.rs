//! Error types for immerse-core.

use thiserror::Error;

/// Main error type for the immerse-core library.
#[derive(Error, Debug)]
pub enum Error {
    // Config errors
    #[error("Failed to load config '{0}': {1}")]
    ConfigLoad(String, String),

    #[error("Failed to parse config '{0}': {1}")]
    ConfigParse(String, String),

    #[error("Config validation error in '{0}': {1}")]
    ConfigValidation(String, String),

    #[error("Failed to acquire cache lock")]
    CacheLock,

    // Sound engine errors
    #[error("No audio player available (tried ffplay, paplay, aplay)")]
    NoAudioPlayer,

    #[error("Sound file not found: {0}")]
    SoundFileNotFound(String),

    #[error("Failed to play sound: {0}")]
    SoundPlayback(String),

    #[error("Sound config not found: {0}")]
    SoundConfNotFound(String),

    // Spotify errors
    #[error("Spotify not configured")]
    SpotifyNotConfigured,

    #[error("Spotify authentication failed: {0}")]
    SpotifyAuth(String),

    #[error("Spotify API error: {0}")]
    SpotifyApi(String),

    #[error("No Spotify device available")]
    SpotifyNoDevice,

    // Lights errors
    #[error("No bulbs configured")]
    NoBulbsConfigured,

    #[error("Failed to send command to bulb {0}: {1}")]
    BulbCommand(String, String),

    #[error("Invalid light group: {0}")]
    InvalidLightGroup(String),

    // Atmosphere errors
    #[error("Failed to download sound from {0}: {1}")]
    AtmosphereDownload(String, String),

    #[error("Failed to start atmosphere playback: {0}")]
    AtmospherePlayback(String),

    // Daemon errors
    #[error("Daemon communication error: {0}")]
    DaemonComm(String),

    #[error("Daemon not running")]
    DaemonNotRunning,

    // Generic errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

/// Result type alias using our Error type.
pub type Result<T> = std::result::Result<T, Error>;
