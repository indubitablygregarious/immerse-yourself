//! Lights engine for controlling WIZ smart bulbs.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::config::{AnimationConfig, LightGroupConfig, RgbGroupConfig, SceneGroupConfig};
use crate::error::{Error, Result};

/// WIZ bulb UDP port.
const WIZ_PORT: u16 = 38899;

/// Lights engine for controlling WIZ smart bulbs with animations.
pub struct LightsEngine {
    groups: HashMap<String, Vec<SocketAddr>>,
    animation_handle: Option<JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
    current_config: Arc<Mutex<Option<AnimationConfig>>>,
}

impl LightsEngine {
    /// Creates a new lights engine with the specified bulb groups.
    pub fn new(groups: HashMap<String, Vec<String>>) -> Result<Self> {
        // Convert IP strings to SocketAddrs
        let mut addr_groups = HashMap::new();
        for (name, ips) in groups {
            let addrs: Vec<SocketAddr> = ips
                .iter()
                .filter_map(|ip| format!("{}:{}", ip, WIZ_PORT).parse().ok())
                .collect();
            if !addrs.is_empty() {
                addr_groups.insert(name, addrs);
            }
        }

        Ok(Self {
            groups: addr_groups,
            animation_handle: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            current_config: Arc::new(Mutex::new(None)),
        })
    }

    /// Creates a lights engine from a wizbulb.ini config file.
    pub fn from_config_file(path: &str) -> Result<Self> {
        let config = ini::Ini::load_from_file(path)
            .map_err(|e| Error::ConfigLoad(path.to_string(), e.to_string()))?;

        let section = config.section(Some("DEFAULT")).ok_or_else(|| {
            Error::ConfigLoad(path.to_string(), "Missing DEFAULT section".to_string())
        })?;

        let mut groups = HashMap::new();

        for group_name in ["backdrop_bulbs", "overhead_bulbs", "battlefield_bulbs"] {
            if let Some(bulbs_str) = section.get(group_name) {
                let ips: Vec<String> = bulbs_str
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                if !ips.is_empty() {
                    // Map config names to animation group names
                    let name = group_name.strip_suffix("_bulbs").unwrap_or(group_name);
                    groups.insert(name.to_string(), ips);
                }
            }
        }

        Self::new(groups)
    }

    /// Starts the animation loop with the given configuration.
    pub async fn start(&mut self, config: AnimationConfig) -> Result<()> {
        // Stop any existing animation
        self.stop().await?;

        // Store config
        {
            let mut current = self.current_config.lock().await;
            *current = Some(config.clone());
        }

        // Reset stop flag
        self.stop_flag.store(false, Ordering::SeqCst);

        // Clone what we need for the animation task
        let groups = self.groups.clone();
        let stop_flag = Arc::clone(&self.stop_flag);
        let current_config = Arc::clone(&self.current_config);

        // Spawn animation task
        let handle = tokio::spawn(async move {
            animation_loop(groups, stop_flag, current_config).await;
        });

        self.animation_handle = Some(handle);
        Ok(())
    }

    /// Stops the animation loop.
    pub async fn stop(&mut self) -> Result<()> {
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(handle) = self.animation_handle.take() {
            // Give it a moment to stop gracefully
            tokio::time::sleep(Duration::from_millis(100)).await;
            handle.abort();
        }

        // Clear config
        {
            let mut current = self.current_config.lock().await;
            *current = None;
        }

        Ok(())
    }

    /// Updates the animation configuration without stopping.
    pub async fn update_config(&self, config: AnimationConfig) -> Result<()> {
        let mut current = self.current_config.lock().await;
        *current = Some(config);
        Ok(())
    }

    /// Sets all bulbs to soft warm white at 70% brightness (for shutdown).
    pub async fn set_warm_white(&self) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(Error::Io)?;

        // Use WIZ's built-in Warm White scene (scene 11) at 70% brightness
        // This gives a true warm white color from the bulb's warm white LEDs
        let pilot = WizPilot {
            method: "setPilot".to_string(),
            params: WizParams {
                r: None,
                g: None,
                b: None,
                dimming: Some(70), // 70% brightness on exit
                scene_id: Some(11), // Scene 11 = Warm White
                speed: None,
            },
        };

        let data = serde_json::to_vec(&pilot)
            .map_err(|e| Error::Other(e.to_string()))?;

        for addrs in self.groups.values() {
            for addr in addrs {
                let _ = socket.send_to(&data, addr);
            }
        }

        Ok(())
    }

    /// Returns whether the engine has any configured bulbs.
    pub fn has_bulbs(&self) -> bool {
        !self.groups.is_empty()
    }

    /// Returns the number of configured bulbs.
    pub fn bulb_count(&self) -> usize {
        self.groups.values().map(|v| v.len()).sum()
    }
}

/// Animation loop that runs in a background task.
async fn animation_loop(
    groups: HashMap<String, Vec<SocketAddr>>,
    stop_flag: Arc<AtomicBool>,
    config: Arc<Mutex<Option<AnimationConfig>>>,
) {
    use rand::SeedableRng;
    let mut rng = rand::rngs::StdRng::from_entropy();

    // Create socket for this animation loop
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => {
            let _ = s.set_nonblocking(true);
            s
        }
        Err(e) => {
            tracing::error!("Failed to create UDP socket: {}", e);
            return;
        }
    };

    // Track pilots for inheritance
    let mut backdrop_pilot: Option<WizPilot> = None;
    let mut overhead_pilot: Option<WizPilot> = None;

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        // Get current config
        let current_config = {
            let guard = config.lock().await;
            guard.clone()
        };

        let Some(anim_config) = current_config else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };

        // First, generate backdrop pilot (if exists) for inheritance
        if let Some(group_config) = anim_config.groups.get("backdrop") {
            if let Some(pilot) = generate_pilot(group_config, &mut rng) {
                backdrop_pilot = Some(pilot);
            }
        }

        // Generate overhead pilot (if exists) for inheritance
        if let Some(group_config) = anim_config.groups.get("overhead") {
            if let Some(pilot) = generate_pilot(group_config, &mut rng) {
                overhead_pilot = Some(pilot);
            }
        }

        // Process each group
        for (group_name, group_config) in &anim_config.groups {
            // Get matching bulbs
            let addrs = match groups.get(group_name) {
                Some(a) => a,
                None => continue,
            };

            // Generate pilot command based on config type
            let pilot = match group_config {
                LightGroupConfig::InheritBackdrop => {
                    backdrop_pilot.clone()
                }
                LightGroupConfig::InheritOverhead => {
                    overhead_pilot.clone()
                }
                LightGroupConfig::Off => {
                    // Turn off: set to black with 0 brightness
                    Some(WizPilot {
                        method: "setPilot".to_string(),
                        params: WizParams {
                            r: Some(0),
                            g: Some(0),
                            b: Some(0),
                            dimming: Some(0),
                            scene_id: None,
                            speed: None,
                        },
                    })
                }
                _ => generate_pilot(group_config, &mut rng),
            };

            // Fire-and-forget: send to all bulbs in group
            if let Some(pilot) = pilot {
                if let Ok(data) = serde_json::to_vec(&pilot) {
                    for addr in addrs {
                        // Non-blocking send, ignore errors
                        let _ = socket.send_to(&data, addr);
                    }
                }
            }
        }

        // Sleep for cycletime
        let sleep_ms = (anim_config.cycletime * 1000.0) as u64;
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }
}

/// Generates a pilot command for a light group config.
fn generate_pilot(config: &LightGroupConfig, rng: &mut impl Rng) -> Option<WizPilot> {
    match config {
        LightGroupConfig::Rgb(rgb_config) => {
            Some(generate_rgb_pilot(rgb_config, rng))
        }
        LightGroupConfig::Scene(scene_config) => {
            Some(generate_scene_pilot(scene_config, rng))
        }
        LightGroupConfig::Off => {
            Some(WizPilot {
                method: "setPilot".to_string(),
                params: WizParams {
                    r: Some(0),
                    g: Some(0),
                    b: Some(0),
                    dimming: Some(0),
                    scene_id: None,
                    speed: None,
                },
            })
        }
        LightGroupConfig::InheritBackdrop | LightGroupConfig::InheritOverhead => {
            // These are handled specially in the animation loop
            None
        }
    }
}

/// Generates an RGB pilot command with random variance.
fn generate_rgb_pilot(config: &RgbGroupConfig, rng: &mut impl Rng) -> WizPilot {
    let [base_r, base_g, base_b] = config.rgb.base;
    let [var_r, var_g, var_b] = config.rgb.variance;

    // Apply variance
    let r = apply_variance(base_r, var_r, rng);
    let g = apply_variance(base_g, var_g, rng);
    let b = apply_variance(base_b, var_b, rng);

    // Random brightness in range
    let brightness = if config.brightness.max > config.brightness.min {
        rng.gen_range(config.brightness.min..=config.brightness.max)
    } else {
        config.brightness.min
    };

    // Check for flash
    let (final_r, final_g, final_b) = if let Some(ref flash) = config.flash {
        if rng.gen::<f32>() < flash.probability {
            if let Some([fr, fg, fb]) = flash.color {
                (fr, fg, fb)
            } else {
                (255, 255, 255) // Default flash is white
            }
        } else {
            (r, g, b)
        }
    } else {
        (r, g, b)
    };

    WizPilot {
        method: "setPilot".to_string(),
        params: WizParams {
            r: Some(final_r),
            g: Some(final_g),
            b: Some(final_b),
            dimming: Some(brightness),
            scene_id: None,
            speed: None,
        },
    }
}

/// Generates a scene pilot command.
fn generate_scene_pilot(config: &SceneGroupConfig, rng: &mut impl Rng) -> WizPilot {
    // Determine scene ID
    let scene_id = if let Some(ref scenes) = config.scenes {
        // Pick random scene from list
        if scenes.ids.is_empty() {
            5 // Default to scene 5 (candle/fireplace)
        } else {
            scenes.ids[rng.gen_range(0..scenes.ids.len())]
        }
    } else if let Some(id) = config.scene_id {
        id
    } else {
        5 // Default
    };

    // Determine speed
    let speed = if let Some(ref scenes) = config.scenes {
        rng.gen_range(scenes.speed_min..=scenes.speed_max)
    } else {
        config.speed.unwrap_or(100)
    };

    // Determine brightness
    let dimming = if let Some(ref brightness) = config.brightness {
        if brightness.max > brightness.min {
            rng.gen_range(brightness.min..=brightness.max)
        } else {
            brightness.min
        }
    } else {
        100
    };

    WizPilot {
        method: "setPilot".to_string(),
        params: WizParams {
            r: None,
            g: None,
            b: None,
            dimming: Some(dimming),
            scene_id: Some(scene_id),
            speed: Some(speed),
        },
    }
}

/// Applies random variance to a color component.
fn apply_variance(base: u8, variance: u8, rng: &mut impl Rng) -> u8 {
    if variance == 0 {
        return base;
    }

    let var = variance as i16;
    let offset = rng.gen_range(-var..=var);
    let result = (base as i16 + offset).clamp(0, 255);
    result as u8
}

/// WIZ bulb command structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WizPilot {
    method: String,
    params: WizParams,
}

/// WIZ bulb parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WizParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    r: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    g: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    b: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimming: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "sceneId")]
    scene_id: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<u8>,
}

impl Drop for LightsEngine {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_variance() {
        let mut rng = rand::thread_rng();

        // No variance
        assert_eq!(apply_variance(128, 0, &mut rng), 128);

        // With variance, result should be within bounds
        for _ in 0..100 {
            let result = apply_variance(128, 20, &mut rng);
            assert!(result >= 108 && result <= 148);
        }

        // Edge cases: shouldn't overflow
        for _ in 0..100 {
            let result = apply_variance(250, 20, &mut rng);
            assert!(result <= 255);

            let result = apply_variance(5, 20, &mut rng);
            assert!(result <= 255); // Can't go below 0, clamped
        }
    }

    #[test]
    fn test_wiz_pilot_serialization() {
        let pilot = WizPilot {
            method: "setPilot".to_string(),
            params: WizParams {
                r: Some(255),
                g: Some(0),
                b: Some(128),
                dimming: Some(100),
                scene_id: None,
                speed: None,
            },
        };

        let json = serde_json::to_string(&pilot).unwrap();
        assert!(json.contains("\"method\":\"setPilot\""));
        assert!(json.contains("\"r\":255"));
        assert!(!json.contains("sceneId")); // Should be skipped when None
    }
}
