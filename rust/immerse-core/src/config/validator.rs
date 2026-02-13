//! Configuration validation.

use crate::config::types::{AnimationConfig, EnvironmentConfig, LightGroupConfig};
use crate::error::{Error, Result};

/// Validator for environment configurations.
pub struct ConfigValidator;

impl ConfigValidator {
    /// Creates a new validator.
    pub fn new() -> Self {
        Self
    }

    /// Validates an environment configuration.
    pub fn validate(&self, config: &EnvironmentConfig) -> Result<()> {
        self.validate_name(config)?;
        self.validate_category(config)?;

        if let Some(ref lights) = config.engines.lights {
            if lights.enabled {
                if let Some(ref animation) = lights.animation {
                    self.validate_animation(animation)?;
                }
            }
        }

        if let Some(ref sound) = config.engines.sound {
            if sound.enabled && !sound.file.is_empty() {
                self.validate_sound_file(&sound.file)?;
            }
        }

        if let Some(ref spotify) = config.engines.spotify {
            if spotify.enabled && !spotify.context_uri.is_empty() {
                self.validate_spotify_uri(&spotify.context_uri)?;
            }
        }

        Ok(())
    }

    fn validate_name(&self, config: &EnvironmentConfig) -> Result<()> {
        if config.name.is_empty() {
            return Err(Error::ConfigValidation(
                "name".to_string(),
                "Name cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_category(&self, config: &EnvironmentConfig) -> Result<()> {
        if config.category.is_empty() {
            return Err(Error::ConfigValidation(
                "category".to_string(),
                "Category cannot be empty".to_string(),
            ));
        }
        // Categories are now dynamic - no validation needed
        Ok(())
    }

    fn validate_animation(&self, animation: &AnimationConfig) -> Result<()> {
        if animation.cycletime <= 0.0 {
            return Err(Error::ConfigValidation(
                "animation.cycletime".to_string(),
                "Cycletime must be positive".to_string(),
            ));
        }

        for (name, group) in &animation.groups {
            self.validate_light_group(name, group)?;
        }

        Ok(())
    }

    fn validate_light_group(&self, name: &str, group: &LightGroupConfig) -> Result<()> {
        match group {
            LightGroupConfig::Rgb(rgb) => {
                if rgb.brightness.min > rgb.brightness.max {
                    return Err(Error::ConfigValidation(
                        format!("groups.{}.brightness", name),
                        "min brightness cannot be greater than max".to_string(),
                    ));
                }
            }
            LightGroupConfig::Scene(scene) => {
                if let Some(ref brightness) = scene.brightness {
                    if brightness.min > brightness.max {
                        return Err(Error::ConfigValidation(
                            format!("groups.{}.brightness", name),
                            "min brightness cannot be greater than max".to_string(),
                        ));
                    }
                }
            }
            LightGroupConfig::Off
            | LightGroupConfig::InheritBackdrop
            | LightGroupConfig::InheritOverhead => {
                // No validation needed
            }
        }

        Ok(())
    }

    fn validate_sound_file(&self, file: &str) -> Result<()> {
        if file.is_empty() {
            return Err(Error::ConfigValidation(
                "sound.file".to_string(),
                "Sound file path cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_spotify_uri(&self, uri: &str) -> Result<()> {
        if !uri.starts_with("spotify:") {
            return Err(Error::ConfigValidation(
                "spotify.context_uri".to_string(),
                format!("Invalid Spotify URI: '{}'. Must start with 'spotify:'", uri),
            ));
        }
        Ok(())
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::*;
    use std::collections::HashMap;

    fn minimal_config() -> EnvironmentConfig {
        EnvironmentConfig {
            name: "Test".to_string(),
            category: "test".to_string(),
            description: None,
            icon: None,
            metadata: None,
            engines: EnginesConfig::default(),
            time_variants: None,
            source_path: None,
        }
    }

    #[test]
    fn test_validate_empty_name() {
        let validator = ConfigValidator::new();
        let mut config = minimal_config();
        config.name = "".to_string();

        let result = validator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_cycletime() {
        let validator = ConfigValidator::new();
        let mut config = minimal_config();
        config.engines.lights = Some(LightsConfig {
            enabled: true,
            animation: Some(AnimationConfig {
                cycletime: -1.0,
                groups: HashMap::from([(
                    "test".to_string(),
                    LightGroupConfig::Rgb(RgbGroupConfig {
                        rgb: RgbConfig {
                            base: [255, 0, 0],
                            variance: [0, 0, 0],
                        },
                        brightness: BrightnessConfig { min: 100, max: 200 },
                        flash: None,
                    }),
                )]),
            }),
        });

        let result = validator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_spotify_uri() {
        let validator = ConfigValidator::new();
        let mut config = minimal_config();
        config.engines.spotify = Some(SpotifyConfig {
            enabled: true,
            context_uri: "not-a-spotify-uri".to_string(),
            offset: None,
        });

        let result = validator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_custom_category() {
        let validator = ConfigValidator::new();
        let mut config = minimal_config();
        config.category = "my_custom_category".to_string();

        let result = validator.validate(&config);
        assert!(result.is_ok());
    }
}
