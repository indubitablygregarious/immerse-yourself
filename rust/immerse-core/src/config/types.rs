//! Configuration types for environment definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete environment configuration loaded from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentConfig {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub metadata: Option<Metadata>,
    #[serde(default)]
    pub engines: EnginesConfig,
    #[serde(default)]
    pub time_variants: Option<HashMap<String, TimeVariant>>,
    /// Full path to the source YAML file this config was loaded from.
    /// Used for resolving time variants in multi-directory setups.
    #[serde(skip)]
    pub source_path: Option<std::path::PathBuf>,
}

impl EnvironmentConfig {
    /// Returns true if this config has any enabled engines.
    pub fn has_any_engine(&self) -> bool {
        self.engines.sound.as_ref().map_or(false, |s| s.enabled)
            || self.engines.spotify.as_ref().map_or(false, |s| s.enabled)
            || self.engines.atmosphere.as_ref().map_or(false, |a| a.enabled)
            || self.engines.lights.as_ref().map_or(false, |l| l.enabled)
    }

    /// Returns true if this config has lights enabled with animation.
    pub fn has_lights(&self) -> bool {
        self.engines
            .lights
            .as_ref()
            .map_or(false, |l| l.enabled && l.animation.is_some())
    }

    /// Returns true if this config has Spotify enabled with a valid URI.
    pub fn has_spotify(&self) -> bool {
        self.engines
            .spotify
            .as_ref()
            .map_or(false, |s| s.enabled && !s.context_uri.is_empty())
    }

    /// Returns true if this is a sound-only config (no lights or atmosphere).
    pub fn is_sound_only(&self) -> bool {
        self.engines.sound.as_ref().map_or(false, |s| s.enabled)
            && !self.has_lights()
            && !self.engines.atmosphere.as_ref().map_or(false, |a| a.enabled)
    }

    /// Returns true if this is a loop sound (atmosphere toggle).
    pub fn is_loop_sound(&self) -> bool {
        // Check metadata.loop_sound
        if self.metadata.as_ref().map_or(false, |m| m.loop_sound) {
            return true;
        }
        // Check engines.sound.is_loop
        if self.engines.sound.as_ref().map_or(false, |s| s.is_loop) {
            return true;
        }
        false
    }

    /// Returns true if this is any kind of sound effect (loop or one-shot).
    /// Sound effects have sound enabled but no lights/spotify/atmosphere.
    pub fn is_sound_effect(&self) -> bool {
        let has_sound = self
            .engines
            .sound
            .as_ref()
            .map_or(false, |s| s.enabled);
        let has_lights = self
            .engines
            .lights
            .as_ref()
            .map_or(false, |l| l.enabled);
        let has_spotify = self
            .engines
            .spotify
            .as_ref()
            .map_or(false, |s| s.enabled && !s.context_uri.is_empty());
        let has_atmosphere = self
            .engines
            .atmosphere
            .as_ref()
            .map_or(false, |a| a.enabled);

        has_sound && !has_lights && !has_spotify && !has_atmosphere
    }
}

/// Environment metadata for search and categorization.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Metadata {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub intensity: Option<String>,
    #[serde(default)]
    pub suitable_for: Vec<String>,
    /// Whether this is a loop sound (atmosphere toggle).
    #[serde(default, rename = "loop")]
    pub loop_sound: bool,
}

/// Container for all engine configurations.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnginesConfig {
    #[serde(default)]
    pub sound: Option<SoundConfig>,
    #[serde(default)]
    pub spotify: Option<SpotifyConfig>,
    #[serde(default)]
    pub atmosphere: Option<AtmosphereConfig>,
    #[serde(default)]
    pub lights: Option<LightsConfig>,
}

/// Sound engine configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SoundConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Path to sound file or sound_conf reference (e.g., "sound_conf:transition").
    #[serde(default)]
    pub file: String,
    /// Whether this is a loop sound (plays as atmosphere, can be toggled).
    #[serde(default, rename = "loop")]
    pub is_loop: bool,
}

fn default_true() -> bool {
    true
}

/// Spotify engine configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpotifyConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Spotify context URI (playlist, album, etc.). If empty, Spotify won't activate.
    #[serde(default)]
    pub context_uri: String,
    /// Optional offset to start at a specific track.
    #[serde(default)]
    pub offset: Option<SpotifyOffset>,
}

/// Spotify playback offset.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpotifyOffset {
    #[serde(default)]
    pub position: Option<u32>,
    #[serde(default)]
    pub uri: Option<String>,
}

/// Atmosphere engine configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AtmosphereConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Minimum sounds to play (optional).
    #[serde(default)]
    pub min_sounds: Option<u32>,
    /// Maximum sounds to play (optional).
    #[serde(default)]
    pub max_sounds: Option<u32>,
    /// When true, skip atmosphere if Spotify is connected and the environment has a Spotify URI.
    /// This lets battle environments fall back to built-in CC-BY music only when Spotify is unavailable.
    #[serde(default)]
    pub spotify_fallback: Option<bool>,
    /// List of ambient sound mixes to play.
    #[serde(default)]
    pub mix: Vec<SoundMix>,
}

/// Individual sound in an atmosphere mix.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SoundMix {
    /// Freesound.org URL or local file path.
    pub url: String,
    /// Volume level (0-100).
    #[serde(default = "default_volume")]
    pub volume: u8,
    /// Optional name for display.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether this sound is optional.
    #[serde(default)]
    pub optional: Option<bool>,
    /// Probability of playing (0.0-1.0) for optional sounds.
    #[serde(default)]
    pub probability: Option<f32>,
    /// Maximum duration in seconds before stopping the sound.
    #[serde(default)]
    pub max_duration: Option<u32>,
    /// Fade-out duration in seconds. When set, the sound will gradually fade out
    /// over this many seconds before stopping (used with max_duration for smooth transitions).
    #[serde(default)]
    pub fade_duration: Option<u32>,
    /// Pool name for mutually exclusive sounds. Sounds with the same pool name
    /// play one at a time — when one finishes, another from the pool starts randomly.
    #[serde(default)]
    pub pool: Option<String>,
    /// Retrigger configuration for sporadic one-shot playback.
    /// When set, the sound plays once, waits a random delay, then plays again
    /// at slightly varied volume and pitch. Mutually exclusive with `pool`.
    #[serde(default)]
    pub retrigger: Option<RetriggerConfig>,
    /// Start offset in seconds — skips the beginning of the sound file.
    /// Useful for trimming unwanted intros (e.g., voice announcements).
    #[serde(default)]
    pub start_offset: Option<f64>,
}

/// Configuration for retrigger mode — plays a sound once, waits a random delay,
/// then plays again at slightly varied volume and pitch.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetriggerConfig {
    /// Minimum seconds to wait before the next trigger.
    pub min_delay: u32,
    /// Maximum seconds to wait before the next trigger.
    pub max_delay: u32,
    /// Volume variance as ±percentage of base volume (default: 15).
    #[serde(default = "default_volume_variance")]
    pub volume_variance: u8,
    /// Pitch variance in ±semitones (default: 0.0 = no pitch change).
    #[serde(default)]
    pub pitch_variance: f32,
}

fn default_volume_variance() -> u8 {
    15
}

fn default_volume() -> u8 {
    70
}

/// Lights engine configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LightsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Animation configuration (optional when disabled).
    #[serde(default)]
    pub animation: Option<AnimationConfig>,
}

/// Animation configuration for lights.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnimationConfig {
    /// Cycle time in seconds for animation loop.
    #[serde(default = "default_cycletime")]
    pub cycletime: f32,
    /// Light groups configuration.
    #[serde(default)]
    pub groups: HashMap<String, LightGroupConfig>,
}

fn default_cycletime() -> f32 {
    10.0
}

/// Configuration for a single light group.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LightGroupConfig {
    /// RGB color-based lighting.
    Rgb(RgbGroupConfig),
    /// WIZ scene-based lighting.
    Scene(SceneGroupConfig),
    /// Turn off this light group.
    Off,
    /// Inherit settings from backdrop group.
    InheritBackdrop,
    /// Inherit settings from overhead group.
    InheritOverhead,
}

/// RGB-based light group configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RgbGroupConfig {
    pub rgb: RgbConfig,
    pub brightness: BrightnessConfig,
    #[serde(default)]
    pub flash: Option<FlashConfig>,
}

/// RGB color configuration with variance.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RgbConfig {
    /// Base RGB color [R, G, B].
    pub base: [u8; 3],
    /// Variance to apply randomly [R, G, B].
    #[serde(default)]
    pub variance: [u8; 3],
}

/// Brightness range configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BrightnessConfig {
    #[serde(default)]
    pub min: u8,
    #[serde(default = "default_max_brightness")]
    pub max: u8,
}

fn default_max_brightness() -> u8 {
    255
}

/// Flash effect configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlashConfig {
    /// Probability of flash (0.0 to 1.0).
    #[serde(default)]
    pub probability: f32,
    /// Flash color [R, G, B].
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// Flash brightness.
    #[serde(default)]
    pub brightness: Option<u8>,
    /// Flash duration in seconds.
    #[serde(default)]
    pub duration: Option<f32>,
}

/// Scene-based light group configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SceneGroupConfig {
    /// Scene configuration.
    #[serde(default)]
    pub scenes: Option<ScenesConfig>,
    /// Legacy: single scene ID.
    #[serde(default)]
    pub scene_id: Option<u8>,
    /// Legacy: single speed.
    #[serde(default)]
    pub speed: Option<u8>,
    /// Brightness configuration.
    #[serde(default)]
    pub brightness: Option<BrightnessConfig>,
}

/// Multiple scene configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScenesConfig {
    /// List of scene IDs to cycle through.
    pub ids: Vec<u8>,
    /// Minimum speed (1-200).
    #[serde(default = "default_speed_min")]
    pub speed_min: u8,
    /// Maximum speed (1-200).
    #[serde(default = "default_speed_max")]
    pub speed_max: u8,
}

fn default_speed_min() -> u8 {
    10
}

fn default_speed_max() -> u8 {
    190
}

/// Time variant configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimeVariant {
    /// Reference to another config file.
    #[serde(default)]
    pub config: Option<String>,
    /// Inline overrides.
    #[serde(flatten)]
    pub overrides: HashMap<String, serde_yaml::Value>,
}

/// Available time-of-day options.
/// Python uses 4 periods: morning, daytime, afternoon, evening
/// "daytime" is the default (base config with no time variant overrides applied)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeOfDay {
    Morning,
    Daytime,
    Afternoon,
    Evening,
}

impl TimeOfDay {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeOfDay::Morning => "morning",
            TimeOfDay::Daytime => "daytime",
            TimeOfDay::Afternoon => "afternoon",
            TimeOfDay::Evening => "evening",
        }
    }

    /// Returns all time periods in canonical order.
    pub fn all() -> &'static [TimeOfDay] {
        &[
            TimeOfDay::Morning,
            TimeOfDay::Daytime,
            TimeOfDay::Afternoon,
            TimeOfDay::Evening,
        ]
    }

    /// Returns all time period names in canonical order.
    pub fn all_names() -> &'static [&'static str] {
        &["morning", "daytime", "afternoon", "evening"]
    }

    /// The default time of day (daytime - uses base config).
    pub fn default_time() -> Self {
        TimeOfDay::Daytime
    }

    /// Parse from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "morning" => Some(TimeOfDay::Morning),
            "daytime" => Some(TimeOfDay::Daytime),
            "afternoon" => Some(TimeOfDay::Afternoon),
            "evening" => Some(TimeOfDay::Evening),
            _ => None,
        }
    }
}

impl Default for TimeOfDay {
    fn default() -> Self {
        TimeOfDay::Daytime
    }
}

impl std::fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_config_has_lights() {
        let config = EnvironmentConfig {
            name: "Test".to_string(),
            category: "test".to_string(),
            description: None,
            icon: None,
            metadata: None,
            engines: EnginesConfig {
                lights: Some(LightsConfig {
                    enabled: true,
                    animation: Some(AnimationConfig {
                        cycletime: 10.0,
                        groups: HashMap::new(),
                    }),
                }),
                ..Default::default()
            },
            time_variants: None,
            source_path: None,
        };
        assert!(config.has_lights());
    }

    #[test]
    fn test_time_of_day_display() {
        assert_eq!(TimeOfDay::Morning.to_string(), "morning");
        assert_eq!(TimeOfDay::Daytime.to_string(), "daytime");
        assert_eq!(TimeOfDay::Afternoon.to_string(), "afternoon");
        assert_eq!(TimeOfDay::Evening.to_string(), "evening");
    }

    #[test]
    fn test_time_of_day_default() {
        assert_eq!(TimeOfDay::default(), TimeOfDay::Daytime);
    }

    #[test]
    fn test_time_of_day_from_str() {
        assert_eq!(TimeOfDay::from_str("morning"), Some(TimeOfDay::Morning));
        assert_eq!(TimeOfDay::from_str("daytime"), Some(TimeOfDay::Daytime));
        assert_eq!(TimeOfDay::from_str("afternoon"), Some(TimeOfDay::Afternoon));
        assert_eq!(TimeOfDay::from_str("evening"), Some(TimeOfDay::Evening));
        assert_eq!(TimeOfDay::from_str("invalid"), None);
    }

    #[test]
    fn test_is_loop_sound_via_metadata() {
        let config = EnvironmentConfig {
            name: "Test Loop".to_string(),
            category: "test".to_string(),
            description: None,
            icon: None,
            metadata: Some(Metadata {
                tags: vec![],
                intensity: None,
                suitable_for: vec![],
                loop_sound: true,
            }),
            engines: EnginesConfig::default(),
            time_variants: None,
            source_path: None,
        };
        assert!(config.is_loop_sound());
    }

    #[test]
    fn test_is_loop_sound_via_sound_config() {
        let config = EnvironmentConfig {
            name: "Test Loop".to_string(),
            category: "test".to_string(),
            description: None,
            icon: None,
            metadata: None,
            engines: EnginesConfig {
                sound: Some(SoundConfig {
                    enabled: true,
                    file: "test.wav".to_string(),
                    is_loop: true,
                }),
                ..Default::default()
            },
            time_variants: None,
            source_path: None,
        };
        assert!(config.is_loop_sound());
    }

    #[test]
    fn test_is_not_loop_sound() {
        let config = EnvironmentConfig {
            name: "Test Normal".to_string(),
            category: "test".to_string(),
            description: None,
            icon: None,
            metadata: None,
            engines: EnginesConfig::default(),
            time_variants: None,
            source_path: None,
        };
        assert!(!config.is_loop_sound());
    }

    #[test]
    fn test_parse_loop_sound_yaml() {
        let yaml = r#"
name: "Test Loop Sound"
category: "water"
metadata:
  loop: true
engines:
  sound:
    enabled: true
    file: "test.wav"
    loop: true
"#;
        let config: EnvironmentConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.is_loop_sound());
        assert!(config.metadata.as_ref().unwrap().loop_sound);
        assert!(config.engines.sound.as_ref().unwrap().is_loop);
    }
}
