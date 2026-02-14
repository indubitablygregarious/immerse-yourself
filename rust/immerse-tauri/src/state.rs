//! Application state management for Tauri.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Thread-safe log buffer for capturing tracing output (visible in iOS debug UI).
pub type LogBuffer = Arc<std::sync::Mutex<VecDeque<String>>>;

/// Environment categories - shown BEFORE the "── SOUNDS ──" separator.
/// These contain full environments with lights, spotify, atmosphere, etc.
const ENVIRONMENT_CATEGORIES: &[&str] = &[
    "tavern", "town", "interiors", "travel", "forest", "coastal",
    "dungeon", "combat", "spooky", "relaxation", "celestial",
];

/// Sound categories - shown AFTER the "── SOUNDS ──" separator.
/// These contain sound effects and loop sounds.
const SOUND_CATEGORIES: &[&str] = &[
    "nature", "water", "fire", "wind", "storm", "crowd",
    "footsteps", "reactions", "combat_sfx", "ambient", "creatures",
    "misc", "freesound", "sounds",
];

/// Hidden categories - never shown in the UI.
const HIDDEN_CATEGORIES: &[&str] = &["hidden"];

/// Tag-to-category keyword mappings for auto-categorizing freesound atmosphere sounds.
/// Mirrors the Python `SOUND_CATEGORY_MAPPINGS` in `freesound_manager.py`.
const SOUND_CATEGORY_KEYWORDS: &[(&str, &[&str])] = &[
    ("nature", &[
        "birds", "bird", "insects", "insect", "frogs", "frog", "cicadas", "wildlife",
        "animal", "cricket", "owl", "songbird", "chirp", "chirping", "morning", "park",
        "woodpecker", "forest", "tree", "dawn",
    ]),
    ("water", &[
        "water", "river", "stream", "drip", "splash", "waves", "brook", "waterfall",
        "pond", "lake", "ocean", "sea", "beach", "coastal", "shore",
    ]),
    ("fire", &[
        "fire", "fireplace", "campfire", "crackling", "flames", "burning", "bonfire",
        "hearth", "ember", "torch", "inferno", "roaring",
    ]),
    ("wind", &[
        "wind", "breeze", "gust", "howling", "rustling", "leaves", "windy", "blowing",
        "desert", "sandy", "dune", "arid", "muffled",
    ]),
    ("storm", &[
        "thunder", "lightning", "storm", "thunderstorm", "tempest", "rain", "rainstorm",
        "downpour", "blizzard", "weather",
    ]),
    ("crowd", &[
        "crowd", "people", "chatter", "murmur", "applause", "laughter", "talking",
        "voices", "audience", "bar", "pub", "tavern", "cafe", "town", "market",
        "festival", "seagulls", "carnival", "circus", "children",
    ]),
    ("footsteps", &[
        "footsteps", "walking", "steps", "gravel", "floor", "boots", "running", "feet",
        "sneakers",
    ]),
    ("reactions", &[
        "gasp", "scream", "groan", "sigh", "cough", "sneeze", "bruh", "shock",
        "surprise", "yell", "shout", "war_cry", "yelling",
    ]),
    ("combat_sfx", &[
        "sword", "weapon", "impact", "hit", "slash", "clang", "bone", "metal", "punch",
        "stab", "fight", "battle", "combat", "tribal", "anvil", "unsheath",
    ]),
    ("ambient", &[
        "ambient", "room", "tone", "hum", "drone", "atmosphere", "background", "loop",
        "ambience", "ambiance", "eerie", "spooky", "creepy", "whisper", "dungeon",
        "crypt", "haunted", "library", "clock", "ticking",
    ]),
    ("creatures", &[
        "monster", "creature", "growl", "howl", "roar", "beast", "wolf", "dragon",
        "demonic", "demon", "coyote", "camel", "angel",
    ]),
];

use immerse_core::config::{
    get_available_times_at_path, get_time_variant_engines_at_path, has_time_variants_at_path,
    ConfigLoader, EnginesConfig, EnvironmentConfig, Metadata, SoundConfig, TimeOfDay,
};
use immerse_core::download_queue::{find_downloaded_file, parse_freesound_url};
use immerse_core::engines::{AtmosphereEngine, LightsEngine, SoundEngine, SpotifyEngine};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

/// Thread-safe application state wrapper.
pub struct AppState {
    inner: Arc<Mutex<AppStateInner>>,
    runtime: Runtime,
    log_buffer: LogBuffer,
    /// Generation counter to invalidate stale environment switches.
    env_generation: Arc<AtomicU64>,
}

/// Inner application state (mutable parts).
struct AppStateInner {
    // Paths
    project_root: PathBuf,
    /// Writable directory for freesound downloads.
    /// On desktop: `project_root/freesound.org/`
    /// On iOS: `app_cache_dir/freesound.org/` (writable)
    cache_dir: PathBuf,
    /// User content directory for custom configs, sounds, and sound collections.
    /// Platform-standard app data directory (e.g., ~/.local/share/com.peterlesko.immerseyourself/).
    user_content_dir: Option<PathBuf>,

    // Config
    config_loader: ConfigLoader,
    configs_by_category: HashMap<String, Vec<EnvironmentConfig>>,
    categories: Vec<String>,

    // Current state
    current_category: String,
    active_lights_config: Option<String>,
    active_sound_name: Option<String>,
    active_atmosphere_urls: HashSet<String>,
    atmosphere_volumes: HashMap<String, u8>,
    current_time: TimeOfDay,

    // Search
    search_query: String,
    search_results: Vec<EnvironmentConfig>,

    // Engines
    sound_engine: Arc<SoundEngine>,
    lights_engine: Option<Arc<Mutex<LightsEngine>>>,
    atmosphere_engine: Arc<AtmosphereEngine>,
    spotify_engine: Option<Arc<Mutex<SpotifyEngine>>>,

    // Session flags
    lights_disabled_this_session: bool,
    /// Whether virtual loop config names may need refreshing after downloads.
    needs_name_refresh: bool,
    /// Whether sounds (both sound engine and atmosphere) are currently paused.
    sounds_paused: bool,
    /// URLs started via toggle_loop_sound — survive environment changes.
    active_loop_urls: HashSet<String>,
    /// Whether virtual loop configs need regenerating after background downloads complete.
    needs_loop_regen: bool,
    /// Incremented whenever categories/configs change (e.g., after loop regen, cache clear, reload).
    /// Frontend watches this to know when to re-fetch categories and configs.
    config_version: u64,
}

/// Active state snapshot for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveState {
    pub active_lights_config: Option<String>,
    /// Name of the currently playing sound effect (entry sound or one-shot).
    /// None if no sound is currently playing.
    pub active_sound: Option<String>,
    pub active_atmosphere_urls: Vec<String>,
    /// Display names for active atmosphere sounds (cleaned up for UI).
    pub atmosphere_names: Vec<String>,
    /// Raw names with author info for tooltips (e.g., "Sound Name by Author").
    pub atmosphere_names_with_author: Vec<String>,
    pub atmosphere_volumes: HashMap<String, u8>,
    pub current_time: String,
    pub current_category: String,
    pub lights_available: bool,
    pub spotify_available: bool,
    pub is_downloading: bool,
    pub pending_downloads: usize,
    /// Available time variants for the active lights config.
    /// Empty if no lights config is active or the config has no time variants.
    pub available_times: Vec<String>,
    /// Whether sounds are currently paused (both sound engine and atmosphere).
    pub is_sounds_paused: bool,
    /// Incremented when categories/configs change. Frontend watches this to refresh.
    pub config_version: u64,
}

/// Available time variants for a config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableTimes {
    pub config_name: String,
    pub times: Vec<String>,
    pub has_variants: bool,
}

/// Spotify configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpotifyConfig {
    pub username: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auto_start: String, // "ask", "start_local", "use_remote", "disabled"
    pub is_configured: bool,
}

/// WIZ bulb configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WizBulbConfig {
    pub backdrop_bulbs: String,
    pub overhead_bulbs: String,
    pub battlefield_bulbs: String,
    pub is_configured: bool,
}

/// App settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub ignore_ssl_errors: bool,
    pub spotify_auto_start: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ignore_ssl_errors: false,
            spotify_auto_start: "ask".to_string(),
        }
    }
}

impl AppState {
    /// Creates a new application state.
    pub fn new() -> Self {
        let runtime = Runtime::new().expect("Failed to create tokio runtime");
        let inner = AppStateInner::new(None, None, None);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            runtime,
            log_buffer: Arc::new(std::sync::Mutex::new(VecDeque::new())),
            env_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Creates a new application state with a Tauri resource directory.
    /// On iOS/mobile, the resource directory is where bundled files are placed.
    pub fn new_with_resource_dir(resource_dir: Option<PathBuf>) -> Self {
        let runtime = Runtime::new().expect("Failed to create tokio runtime");
        let inner = AppStateInner::new(resource_dir, None, None);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            runtime,
            log_buffer: Arc::new(std::sync::Mutex::new(VecDeque::new())),
            env_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Creates a new application state with a Tauri resource directory and shared log buffer.
    /// `cache_dir` overrides the freesound download cache location (needed on iOS where
    /// the project root is read-only).
    /// `user_content_dir` is the platform-standard app data directory for user content.
    pub fn new_with_resource_dir_and_log(
        resource_dir: Option<PathBuf>,
        log_buffer: LogBuffer,
        cache_dir: Option<PathBuf>,
        user_content_dir: Option<PathBuf>,
    ) -> Self {
        let runtime = Runtime::new().expect("Failed to create tokio runtime");
        let inner = AppStateInner::new(resource_dir, cache_dir, user_content_dir);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            runtime,
            log_buffer,
            env_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns the user content directory path, if configured.
    pub fn get_user_content_dir(&self) -> Option<String> {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.user_content_dir.as_ref().map(|p| p.display().to_string())
        })
    }

    /// Returns the most recent log entries (up to 500).
    pub fn get_debug_log(&self) -> Vec<String> {
        let buf = self.log_buffer.lock().unwrap_or_else(|e| e.into_inner());
        buf.iter().cloned().collect()
    }

    /// Clears all buffered log entries.
    pub fn clear_debug_log(&self) {
        let mut buf = self.log_buffer.lock().unwrap_or_else(|e| e.into_inner());
        buf.clear();
    }

    /// Gets all category names, ordered with environment categories first,
    /// then sound categories.
    pub fn get_categories(&self) -> Vec<String> {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.get_ordered_categories()
        })
    }

    /// Gets environments for a specific category.
    /// For environment categories, filters out all sound effects (both loop and one-shot).
    /// For sound categories, only returns sound effects (both loop and one-shot).
    pub fn get_environments(&self, category: &str) -> Vec<EnvironmentConfig> {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            let is_sound_cat = inner.is_sound_category(category);

            inner
                .configs_by_category
                .get(category)
                .map(|configs| {
                    configs
                        .iter()
                        .filter(|c| {
                            if is_sound_cat {
                                // Sound category: show all sound effects (loop or one-shot)
                                c.is_sound_effect() || c.is_loop_sound()
                            } else {
                                // Environment category: exclude all sound effects
                                !c.is_sound_effect() && !c.is_loop_sound()
                            }
                        })
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    /// Gets all configs across all categories.
    /// For environment categories, filters out all sound effects (both loop and one-shot).
    /// For sound categories, only returns sound effects (both loop and one-shot).
    pub fn get_all_configs(&self) -> HashMap<String, Vec<EnvironmentConfig>> {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;

            inner
                .configs_by_category
                .iter()
                .map(|(category, configs)| {
                    let is_sound_cat = inner.is_sound_category(category);
                    let filtered: Vec<EnvironmentConfig> = configs
                        .iter()
                        .filter(|c| {
                            if is_sound_cat {
                                // Sound category: show all sound effects (loop or one-shot)
                                c.is_sound_effect() || c.is_loop_sound()
                            } else {
                                // Environment category: exclude all sound effects
                                !c.is_sound_effect() && !c.is_loop_sound()
                            }
                        })
                        .cloned()
                        .collect();
                    (category.clone(), filtered)
                })
                .filter(|(_, configs)| !configs.is_empty())
                .collect()
        })
    }

    /// Starts an environment by name.
    /// If atmosphere sounds need downloading, spawns a background task that waits
    /// for downloads then switches — the UI thread is never blocked and the old
    /// environment keeps playing until the new one is ready.
    pub fn start_environment(&self, config_name: &str) -> Result<(), String> {
        // Increment generation — invalidates any pending background switch
        let gen = self.env_generation.fetch_add(1, Ordering::SeqCst) + 1;

        self.runtime.block_on(async {
            // Find config and check download status (brief lock)
            let (config, needs_download, atmo_engine, urls) = {
                let inner = self.inner.lock().await;
                let config = inner
                    .configs_by_category
                    .values()
                    .flatten()
                    .find(|c| c.name == config_name)
                    .cloned()
                    .ok_or_else(|| format!("Config not found: {}", config_name))?;

                let (needs, engine, urls) = if let Some(ref atmosphere) = config.engines.atmosphere {
                    if atmosphere.enabled {
                        let engine = Arc::clone(&inner.atmosphere_engine);
                        let uncached: Vec<String> = atmosphere
                            .mix
                            .iter()
                            .filter(|s| !engine.is_url_cached(&s.url))
                            .map(|s| s.url.clone())
                            .collect();
                        if !uncached.is_empty() {
                            for url in &uncached {
                                engine.pre_download(url);
                            }
                            (true, Some(engine), uncached)
                        } else {
                            (false, None, vec![])
                        }
                    } else {
                        (false, None, vec![])
                    }
                } else {
                    (false, None, vec![])
                };

                (config, needs, engine, urls)
            }; // lock released

            if needs_download {
                // Downloads needed — spawn background task to wait then switch
                let inner = self.inner.clone();
                let env_gen = Arc::clone(&self.env_generation);
                let atmo_engine = atmo_engine.unwrap();

                tokio::spawn(async move {
                    // Wait for all URLs to be cached
                    let max_wait = std::time::Duration::from_secs(90);
                    let start = std::time::Instant::now();
                    loop {
                        if start.elapsed() > max_wait {
                            tracing::warn!(
                                "Timed out waiting for atmosphere downloads for {}",
                                config.name
                            );
                            break;
                        }
                        if env_gen.load(Ordering::SeqCst) != gen {
                            tracing::info!(
                                "Environment switch for '{}' superseded, abandoning",
                                config.name
                            );
                            return;
                        }
                        if urls.iter().all(|url| atmo_engine.is_url_cached(url)) {
                            tracing::info!(
                                "All atmosphere sounds downloaded for {}",
                                config.name
                            );
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    }

                    // Check generation again before starting
                    if env_gen.load(Ordering::SeqCst) != gen {
                        tracing::info!(
                            "Environment switch for '{}' superseded after downloads",
                            config.name
                        );
                        return;
                    }

                    // Now start the environment
                    let mut guard = inner.lock().await;
                    guard.needs_loop_regen = true;
                    guard.start_environment(&config).await;
                });

                Ok(())
            } else {
                // All cached — start immediately
                let mut inner = self.inner.lock().await;
                inner.start_environment(&config).await;
                Ok(())
            }
        })
    }

    /// Toggles a loop sound on/off.
    pub fn toggle_loop_sound(&self, url: &str) -> Result<bool, String> {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;
            inner.toggle_loop_sound_url(url)
        })
    }

    /// Sets the volume for a URL.
    pub fn set_volume(&self, url: &str, volume: u8) -> Result<(), String> {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;
            inner.set_volume(url, volume);
            Ok(())
        })
    }

    /// Stops all lights.
    pub fn stop_lights(&self) -> Result<(), String> {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;
            inner.stop_lights().await;
            Ok(())
        })
    }

    /// Stops all sounds.
    pub fn stop_sounds(&self) -> usize {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.sound_engine.stop_all()
        })
    }

    /// Toggles pause/resume on both sound and atmosphere engines.
    /// Returns the new paused state (true = paused, false = playing).
    pub fn toggle_pause_sounds(&self) -> bool {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;
            if inner.sounds_paused {
                // Resume
                inner.sound_engine.resume_all();
                inner.atmosphere_engine.resume_all();
                inner.sounds_paused = false;
                tracing::info!("Resumed all sounds");
            } else {
                // Pause
                inner.sound_engine.pause_all();
                inner.atmosphere_engine.pause_all();
                inner.sounds_paused = true;
                tracing::info!("Paused all sounds");
            }
            inner.sounds_paused
        })
    }

    /// Stops atmosphere and Spotify.
    pub fn stop_atmosphere(&self) -> Result<(), String> {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;
            inner.stop_atmosphere().await;
            Ok(())
        })
    }

    /// Clears the freesound download cache and reloads configs.
    /// Stops all sounds, deletes all cached audio files, then regenerates
    /// virtual loop configs (which depend on cached filenames for display names).
    /// Returns the number of files deleted.
    pub fn clear_sound_cache(&self) -> Result<usize, String> {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;

            // Stop all playing atmosphere sounds first
            inner.atmosphere_engine.stop_all();
            inner.active_atmosphere_urls.clear();
            inner.atmosphere_volumes.clear();
            inner.sounds_paused = false;

            // Stop one-shot sounds too
            inner.sound_engine.stop_all();
            inner.active_sound_name = None;

            // Clear the cache
            let count = inner.atmosphere_engine.clear_cache()
                .map_err(|e| format!("Failed to clear cache: {}", e))?;

            // Reload configs so virtual loop configs are regenerated
            // without stale cached filenames
            inner.config_loader = inner.build_config_loader();
            inner.configs_by_category = inner.config_loader.load_all().unwrap_or_default();
            let cache_dir = inner.cache_dir.clone();
            AppStateInner::generate_virtual_loop_configs(&mut inner.configs_by_category, &cache_dir);
            let mut categories: Vec<String> = inner.configs_by_category.keys().cloned().collect();
            categories.sort();
            inner.categories = categories;
            inner.needs_name_refresh = true;
            inner.active_lights_config = None;
            inner.config_version += 1;

            tracing::info!("Cleared freesound cache: {} files deleted, configs reloaded (config_version={})", count, inner.config_version);
            Ok(count)
        })
    }

    /// Reloads all YAML configs from disk and regenerates virtual loop configs.
    /// Returns the total number of configs loaded.
    pub fn reload_configs(&self) -> Result<usize, String> {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;

            // Reload all configs from disk
            inner.config_loader = inner.build_config_loader();
            inner.configs_by_category = inner.config_loader.load_all().unwrap_or_default();

            // Regenerate virtual loop configs
            let cache_dir = inner.cache_dir.clone();
            AppStateInner::generate_virtual_loop_configs(&mut inner.configs_by_category, &cache_dir);

            // Re-extract sorted category list
            let mut categories: Vec<String> = inner.configs_by_category.keys().cloned().collect();
            categories.sort();
            inner.categories = categories;

            // Mark names for refresh
            inner.needs_name_refresh = true;

            inner.config_version += 1;

            let total: usize = inner.configs_by_category.values().map(|v| v.len()).sum();
            tracing::info!("Reloaded configs: {} total across {} categories (config_version={})", total, inner.configs_by_category.len(), inner.config_version);
            Ok(total)
        })
    }

    /// Searches configs across all categories.
    pub fn search_configs(&self, query: &str) -> Vec<EnvironmentConfig> {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;
            inner.search_configs(query);
            inner.search_results.clone()
        })
    }

    /// Gets the current active state.
    pub fn get_active_state(&self) -> ActiveState {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;

            // Compute available times for active lights config
            let available_times = if let Some(ref config_name) = inner.active_lights_config {
                let times_info = inner.get_available_times(config_name);
                times_info.times
            } else {
                Vec::new()
            };

            // Clear active sound name if no sounds are currently playing
            if inner.active_sound_name.is_some() && inner.sound_engine.playing_count() == 0 {
                inner.active_sound_name = None;
            }

            // Refresh stale "Sound {id}" names once downloads finish
            if inner.needs_name_refresh && inner.atmosphere_engine.pending_downloads() == 0 {
                inner.refresh_stale_virtual_config_names();
                inner.needs_name_refresh = false;
            }

            // Regenerate virtual loop configs once new downloads complete
            if inner.needs_loop_regen && inner.atmosphere_engine.pending_downloads() == 0 {
                let cache_dir = inner.cache_dir.clone();
                AppStateInner::generate_virtual_loop_configs(
                    &mut inner.configs_by_category,
                    &cache_dir,
                );
                let mut categories: Vec<String> =
                    inner.configs_by_category.keys().cloned().collect();
                categories.sort();
                inner.categories = categories;
                inner.needs_loop_regen = false;
                inner.config_version += 1;
                tracing::info!("Regenerated virtual loop configs after downloads completed (config_version={})", inner.config_version);
            }

            // Get display names for active atmosphere sounds
            let atmosphere_names: Vec<String> = inner
                .active_atmosphere_urls
                .iter()
                .map(|url| inner.get_atmosphere_display_name(url))
                .collect();

            // Get names with author info for tooltips
            let atmosphere_names_with_author: Vec<String> = inner
                .active_atmosphere_urls
                .iter()
                .map(|url| inner.get_atmosphere_name_with_author(url))
                .collect();

            ActiveState {
                active_lights_config: inner.active_lights_config.clone(),
                active_sound: inner.active_sound_name.clone(),
                active_atmosphere_urls: inner.active_atmosphere_urls.iter().cloned().collect(),
                atmosphere_names,
                atmosphere_names_with_author,
                atmosphere_volumes: inner.atmosphere_volumes.clone(),
                current_time: inner.current_time.as_str().to_string(),
                current_category: inner.current_category.clone(),
                lights_available: inner.lights_engine.is_some(),
                spotify_available: inner.spotify_engine.is_some(),
                is_downloading: inner.atmosphere_engine.pending_downloads() > 0,
                pending_downloads: inner.atmosphere_engine.pending_downloads(),
                available_times,
                is_sounds_paused: inner.sounds_paused,
                config_version: inner.config_version,
            }
        })
    }

    /// Cleans up on exit.
    pub fn cleanup(&self) {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;
            inner.cleanup().await;
        });
    }

    /// Gets available time variants for a config.
    pub fn get_available_times(&self, config_name: &str) -> AvailableTimes {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.get_available_times(config_name)
        })
    }

    /// Starts an environment with a specific time variant.
    /// If atmosphere sounds need downloading, spawns a background task that waits
    /// for downloads then switches — the UI thread is never blocked.
    pub fn start_environment_with_time(&self, config_name: &str, time: &str) -> Result<(), String> {
        let gen = self.env_generation.fetch_add(1, Ordering::SeqCst) + 1;

        self.runtime.block_on(async {
            let (needs_download, atmo_engine, urls, config_name_owned, time_owned) = {
                let inner = self.inner.lock().await;
                let base_config = inner
                    .configs_by_category
                    .values()
                    .flatten()
                    .find(|c| c.name == config_name)
                    .cloned();

                if let Some(base_config) = base_config {
                    // Build effective config for pre-download check
                    let config_path = base_config.source_path.clone().unwrap_or_else(|| {
                        let config_dir = inner.config_loader.config_dir();
                        let base_filename =
                            format!("{}.yaml", config_name.to_lowercase().replace(' ', "_"));
                        config_dir.join(&base_filename)
                    });
                    let effective_config = if time == "daytime" {
                        base_config
                    } else if has_time_variants_at_path(&config_path) {
                        if let Some(variant_engines) =
                            get_time_variant_engines_at_path(&config_path, time)
                        {
                            match serde_yaml::from_value::<immerse_core::config::EnginesConfig>(
                                variant_engines,
                            ) {
                                Ok(engines) => {
                                    let mut variant = base_config.clone();
                                    variant.engines = engines;
                                    variant
                                }
                                Err(_) => base_config,
                            }
                        } else {
                            base_config
                        }
                    } else {
                        base_config
                    };

                    // Check and kick off downloads
                    if let Some(ref atmosphere) = effective_config.engines.atmosphere {
                        if atmosphere.enabled {
                            let engine = Arc::clone(&inner.atmosphere_engine);
                            let uncached: Vec<String> = atmosphere
                                .mix
                                .iter()
                                .filter(|s| !engine.is_url_cached(&s.url))
                                .map(|s| s.url.clone())
                                .collect();
                            if !uncached.is_empty() {
                                for url in &uncached {
                                    engine.pre_download(url);
                                }
                                (
                                    true,
                                    Some(engine),
                                    uncached,
                                    config_name.to_string(),
                                    time.to_string(),
                                )
                            } else {
                                (false, None, vec![], config_name.to_string(), time.to_string())
                            }
                        } else {
                            (false, None, vec![], config_name.to_string(), time.to_string())
                        }
                    } else {
                        (false, None, vec![], config_name.to_string(), time.to_string())
                    }
                } else {
                    (false, None, vec![], config_name.to_string(), time.to_string())
                }
            }; // lock released

            if needs_download {
                let inner = self.inner.clone();
                let env_gen = Arc::clone(&self.env_generation);
                let atmo_engine = atmo_engine.unwrap();

                tokio::spawn(async move {
                    let max_wait = std::time::Duration::from_secs(90);
                    let start = std::time::Instant::now();
                    loop {
                        if start.elapsed() > max_wait {
                            tracing::warn!(
                                "Timed out waiting for downloads for {} ({})",
                                config_name_owned,
                                time_owned
                            );
                            break;
                        }
                        if env_gen.load(Ordering::SeqCst) != gen {
                            tracing::info!("Environment switch superseded, abandoning");
                            return;
                        }
                        if urls.iter().all(|url| atmo_engine.is_url_cached(url)) {
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    }

                    if env_gen.load(Ordering::SeqCst) != gen {
                        return;
                    }

                    let mut guard = inner.lock().await;
                    guard.needs_loop_regen = true;
                    let _ = guard
                        .start_environment_with_time(&config_name_owned, &time_owned)
                        .await;
                });

                Ok(())
            } else {
                let mut inner = self.inner.lock().await;
                inner.start_environment_with_time(config_name, time).await
            }
        })
    }

    /// Gets categories that are sound-only.
    pub fn get_sound_categories(&self) -> Vec<String> {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.get_sound_categories()
        })
    }

    /// Sets the current time of day.
    pub fn set_current_time(&self, time: &str) -> Result<(), String> {
        self.runtime.block_on(async {
            let mut inner = self.inner.lock().await;
            inner.set_current_time(time)
        })
    }

    /// Triggers the startup environment if one exists.
    /// Looks for a config named "Startup" (case-insensitive), falling back to "Travel".
    /// Routes through the outer `start_environment()` so download-wait-switch logic applies.
    /// Returns the name of the environment that was started, or None if no startup config found.
    pub fn trigger_startup_environment(&self) -> Option<String> {
        // Find startup config name under a brief lock
        let config_name = self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            let mut startup_name: Option<String> = None;
            let mut travel_name: Option<String> = None;
            for configs in inner.configs_by_category.values() {
                for config in configs {
                    let name_lower = config.name.to_lowercase();
                    if name_lower == "startup" {
                        startup_name = Some(config.name.clone());
                        break;
                    } else if name_lower == "travel" && travel_name.is_none() {
                        travel_name = Some(config.name.clone());
                    }
                }
                if startup_name.is_some() {
                    break;
                }
            }
            startup_name.or(travel_name)
        });

        if let Some(ref name) = config_name {
            tracing::info!("Triggering startup environment: {}", name);
            // Use the outer start_environment which handles download-wait-switch
            let _ = self.start_environment(name);
        }
        config_name
    }

    // ========================================================================
    // Settings Methods
    // ========================================================================

    /// Gets the current Spotify configuration.
    pub fn get_spotify_config(&self) -> SpotifyConfig {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.get_spotify_config()
        })
    }

    /// Saves the Spotify configuration.
    pub fn save_spotify_config(&self, config: SpotifyConfig) -> Result<(), String> {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.save_spotify_config(config)
        })
    }

    /// Gets the current WIZ bulb configuration.
    pub fn get_wizbulb_config(&self) -> WizBulbConfig {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.get_wizbulb_config()
        })
    }

    /// Saves the WIZ bulb configuration.
    pub fn save_wizbulb_config(&self, config: WizBulbConfig) -> Result<(), String> {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.save_wizbulb_config(config)
        })
    }

    /// Gets the current app settings.
    pub fn get_app_settings(&self) -> AppSettings {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.get_app_settings()
        })
    }

    /// Saves the app settings.
    pub fn save_app_settings(&self, settings: AppSettings) -> Result<(), String> {
        self.runtime.block_on(async {
            let inner = self.inner.lock().await;
            inner.save_app_settings(settings)
        })
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppStateInner {
    fn new(
        resource_dir: Option<PathBuf>,
        cache_dir_override: Option<PathBuf>,
        user_content_dir: Option<PathBuf>,
    ) -> Self {
        // Determine project root by finding where env_conf/ is located
        let project_root = Self::find_project_root(resource_dir);

        // Determine writable cache directory for freesound downloads.
        // On iOS the project root is inside the read-only app bundle, so we
        // need an explicit writable path (Tauri's app_cache_dir).
        let cache_dir = cache_dir_override.unwrap_or_else(|| project_root.join("freesound.org"));
        tracing::info!("Freesound cache dir: {:?}", cache_dir);

        // Initialize user content directory structure if configured
        if let Some(ref ucd) = user_content_dir {
            Self::init_user_content_dir(ucd);
            tracing::info!("User content dir: {:?}", ucd);
        }

        // Create config loader (with user content dir if available)
        let config_dir = project_root.join("env_conf");
        let config_loader = if let Some(ref ucd) = user_content_dir {
            let user_env_conf = ucd.join("env_conf");
            ConfigLoader::new_with_dirs(vec![config_dir, user_env_conf])
        } else {
            ConfigLoader::new(&config_dir)
        };

        // Load all configs
        let mut configs_by_category = config_loader.load_all().unwrap_or_default();

        // Generate virtual loop sound configs from atmosphere mix URLs
        Self::generate_virtual_loop_configs(&mut configs_by_category, &cache_dir);

        // Extract sorted category list
        let mut categories: Vec<String> = configs_by_category.keys().cloned().collect();
        categories.sort();

        // Default to first category
        let current_category = categories.first().cloned().unwrap_or_default();

        // Create engines with the writable cache directory
        let mut sound_engine = SoundEngine::new_with_cache_dir(project_root.clone(), cache_dir.clone());
        if let Some(ref ucd) = user_content_dir {
            sound_engine.set_user_content_dir(ucd.clone());
        }
        let sound_engine = Arc::new(sound_engine);
        let atmosphere_engine = Arc::new(AtmosphereEngine::new_with_cache_dir(&cache_dir));

        // Try to load lights engine from config
        let lights_engine = if project_root.join(".wizbulb.ini").exists() {
            match LightsEngine::from_config_file(
                project_root.join(".wizbulb.ini").to_str().unwrap(),
            ) {
                Ok(engine) => {
                    tracing::info!("Loaded WIZ bulb configuration");
                    Some(Arc::new(Mutex::new(engine)))
                }
                Err(e) => {
                    tracing::warn!("Failed to load WIZ bulb config: {}", e);
                    None
                }
            }
        } else {
            tracing::info!("No .wizbulb.ini found, lights disabled");
            None
        };

        // Try to load Spotify engine
        let spotify_engine = if project_root.join(".spotify.ini").exists() {
            match immerse_core::engines::SpotifyCredentials::from_config_file(
                project_root.join(".spotify.ini").to_str().unwrap(),
            ) {
                Ok(creds) if creds.is_configured() => {
                    let cache_path = project_root.join(".cache");
                    let engine = SpotifyEngine::new(creds, cache_path);
                    tracing::info!("Loaded Spotify configuration");
                    Some(Arc::new(Mutex::new(engine)))
                }
                Ok(_) => {
                    tracing::info!("Spotify credentials not fully configured");
                    None
                }
                Err(e) => {
                    tracing::warn!("Failed to load Spotify config: {}", e);
                    None
                }
            }
        } else {
            tracing::info!("No .spotify.ini found, Spotify disabled");
            None
        };

        Self {
            project_root,
            cache_dir,
            user_content_dir,
            config_loader,
            configs_by_category,
            categories,
            current_category,
            active_lights_config: None,
            active_sound_name: None,
            active_atmosphere_urls: HashSet::new(),
            atmosphere_volumes: HashMap::new(),
            current_time: TimeOfDay::default(),
            search_query: String::new(),
            search_results: Vec::new(),
            sound_engine,
            lights_engine,
            atmosphere_engine,
            spotify_engine,
            lights_disabled_this_session: false,
            needs_name_refresh: true,
            sounds_paused: false,
            active_loop_urls: HashSet::new(),
            needs_loop_regen: false,
            config_version: 0,
        }
    }

    /// Creates the user content directory structure and README on first launch.
    fn init_user_content_dir(dir: &Path) {
        use std::fs;

        let subdirs = ["env_conf", "sound_conf", "sounds"];
        for subdir in &subdirs {
            let path = dir.join(subdir);
            if let Err(e) = fs::create_dir_all(&path) {
                tracing::warn!("Failed to create user content subdir {:?}: {}", path, e);
            }
        }

        let readme_path = dir.join("README.md");
        if !readme_path.exists() {
            let readme = "\
# User Content Directory

Place custom environments, sound collections, and audio files here.
Content is loaded alongside built-in configs.

## Structure

    env_conf/    \u{2014} Environment YAML configs (same schema as built-in)
    sound_conf/  \u{2014} Sound collection YAML configs
    sounds/      \u{2014} Audio files (.wav, .mp3, .ogg, .opus, .flac)

## Tips

- Configs with the same filename as built-in ones override them
- Use any category name \u{2014} it appears in the sidebar automatically
- Reference local sounds as: file: \"sounds/myfile.wav\"
- Reference freesound URLs as: url: \"https://freesound.org/...\"
- After adding files, use Settings > Downloads > Reload Configs to pick them up
";
            if let Err(e) = fs::write(&readme_path, readme) {
                tracing::warn!("Failed to write user content README: {}", e);
            }
        }
    }

    /// Builds a ConfigLoader using the project root and optional user content directory.
    fn build_config_loader(&self) -> ConfigLoader {
        let config_dir = self.project_root.join("env_conf");
        if let Some(ref ucd) = self.user_content_dir {
            let user_env_conf = ucd.join("env_conf");
            ConfigLoader::new_with_dirs(vec![config_dir, user_env_conf])
        } else {
            ConfigLoader::new(&config_dir)
        }
    }

    /// Generates virtual loop sound configs from atmosphere mix URLs in all environment configs.
    ///
    /// For each unique freesound URL found in atmosphere mixes, creates a virtual
    /// `EnvironmentConfig` that appears as a toggleable loop sound in the appropriate
    /// sound category. This allows users to individually play/stop atmosphere sounds
    /// from the sidebar.
    ///
    /// Skips URLs that already have an existing loop config (to avoid duplicates).
    fn generate_virtual_loop_configs(
        configs_by_category: &mut HashMap<String, Vec<EnvironmentConfig>>,
        cache_dir: &std::path::Path,
    ) {
        // First, collect all URLs that already have loop configs
        let mut existing_loop_urls: HashSet<String> = HashSet::new();
        for configs in configs_by_category.values() {
            for config in configs {
                if config.is_loop_sound() {
                    if let Some(ref sound) = config.engines.sound {
                        if sound.file.contains("freesound.org") {
                            existing_loop_urls.insert(sound.file.clone());
                        }
                    }
                }
            }
        }

        // Collect all unique atmosphere freesound URLs with their source environment names
        // Use a Vec to track (url, first_source_env_name) while deduplicating by URL
        let mut url_sources: Vec<(String, String, Option<String>)> = Vec::new();
        let mut seen_urls: HashSet<String> = HashSet::new();

        for configs in configs_by_category.values() {
            for config in configs {
                if let Some(ref atmosphere) = config.engines.atmosphere {
                    if atmosphere.enabled {
                        for sound in &atmosphere.mix {
                            if sound.url.contains("freesound.org")
                                && !existing_loop_urls.contains(&sound.url)
                                && !seen_urls.contains(&sound.url)
                            {
                                seen_urls.insert(sound.url.clone());
                                url_sources.push((
                                    sound.url.clone(),
                                    config.name.clone(),
                                    sound.name.clone(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Filter out URLs that are not cached -- uncached sounds cannot play and
        // would appear as useless "Sound {id}" buttons.
        let total_before_filter = url_sources.len();
        url_sources.retain(|(url, _, _)| {
            if let Some((creator, sound_id)) = parse_freesound_url(url) {
                find_downloaded_file(cache_dir, &creator, &sound_id).is_some()
            } else {
                false
            }
        });

        if url_sources.is_empty() {
            if total_before_filter > 0 {
                tracing::info!(
                    "Skipped {} virtual loop configs (no cached files)",
                    total_before_filter
                );
            }
            return;
        }

        tracing::info!(
            "Generating {} virtual loop configs from atmosphere URLs ({} skipped, not cached)",
            url_sources.len(),
            total_before_filter - url_sources.len()
        );

        // Generate virtual configs
        let mut virtual_configs: Vec<(String, EnvironmentConfig)> = Vec::new();

        for (url, source_env, mix_name) in &url_sources {
            // Try to extract a display name from the cached file or mix name
            let display_name =
                Self::get_virtual_config_display_name(url, mix_name.as_deref(), cache_dir);

            // Determine sound category from keywords in the display name and cached filename
            let category = Self::categorize_sound_by_keywords(url, &display_name, cache_dir);

            let config = EnvironmentConfig {
                name: display_name.clone(),
                category: category.clone(),
                description: Some(format!("From: {}", source_env)),
                icon: Some("\u{1F50A}".to_string()), // Speaker emoji
                metadata: Some(Metadata {
                    tags: vec![
                        "freesound".to_string(),
                        "atmosphere".to_string(),
                        "virtual".to_string(),
                    ],
                    intensity: Some("low".to_string()),
                    suitable_for: vec!["ambient".to_string(), "atmosphere".to_string()],
                    loop_sound: true,
                }),
                engines: EnginesConfig {
                    sound: Some(SoundConfig {
                        enabled: true,
                        file: url.clone(),
                        is_loop: true,
                    }),
                    spotify: None,
                    atmosphere: None,
                    lights: None,
                },
                time_variants: None,
                source_path: None,
            };

            virtual_configs.push((category, config));
        }

        // Add virtual configs to their categories
        let mut added_count = 0;
        for (category, config) in virtual_configs {
            configs_by_category
                .entry(category)
                .or_default()
                .push(config);
            added_count += 1;
        }

        tracing::info!(
            "Added {} virtual loop configs to sound categories",
            added_count
        );
    }

    /// Refreshes stale "Sound {id}" display names on virtual loop configs.
    ///
    /// At startup, sounds may not be cached yet so virtual configs get fallback
    /// names like "Sound 12345".  Once files are downloaded, this method resolves
    /// proper names from the cached filenames and updates the configs in place.
    /// Designed to be called cheaply from the polling path (get_active_state).
    fn refresh_stale_virtual_config_names(&mut self) {
        let cache_dir = &self.cache_dir;
        let re = match regex::Regex::new(r"^Sound \d+$") {
            Ok(r) => r,
            Err(_) => return,
        };

        for configs in self.configs_by_category.values_mut() {
            for config in configs.iter_mut() {
                // Only touch virtual loop configs with stale fallback names
                if !config.is_loop_sound() || !re.is_match(&config.name) {
                    continue;
                }
                let url = match config.engines.sound {
                    Some(ref s) if s.file.contains("freesound.org") => &s.file,
                    _ => continue,
                };
                let new_name =
                    Self::get_virtual_config_display_name(url, None, cache_dir);
                if new_name != config.name {
                    tracing::info!(
                        "Refreshed virtual config name: '{}' -> '{}'",
                        config.name,
                        new_name
                    );
                    config.name = new_name;
                }
            }
        }
    }

    /// Determines a display name for a virtual loop config from a freesound URL.
    ///
    /// Tries these sources in order:
    /// 1. The `name` field from the atmosphere mix entry
    /// 2. The cached filename (format: `creator_soundid_soundname.ext`)
    /// 3. Fallback to "Sound {id}"
    fn get_virtual_config_display_name(
        url: &str,
        mix_name: Option<&str>,
        cache_dir: &std::path::Path,
    ) -> String {
        // Option 1: Use the mix name if provided
        if let Some(name) = mix_name {
            if !name.is_empty() {
                return name.to_string();
            }
        }

        // Option 2: Parse cached filename
        if let Some((creator, sound_id)) = parse_freesound_url(url) {
            let prefix = format!("{}_{}_", creator, sound_id);
            if let Ok(entries) = std::fs::read_dir(cache_dir) {
                for entry in entries.flatten() {
                    let filename = entry.file_name().to_string_lossy().to_string();
                    if filename.starts_with(&prefix) {
                        // Extract name part (after creator_id_)
                        let name_with_ext = &filename[prefix.len()..];
                        let name = name_with_ext
                            .rsplit_once('.')
                            .map(|(n, _)| n)
                            .unwrap_or(name_with_ext);
                        // Clean up: replace underscores with spaces
                        let clean = name.replace('_', " ");
                        // Remove "freesound - " prefix
                        let lower = clean.to_lowercase();
                        let clean = if lower.starts_with("freesound - ") {
                            clean[12..].to_string()
                        } else if lower.starts_with("freesound-") {
                            clean[10..].to_string()
                        } else {
                            clean
                        };
                        let clean = clean.trim().to_string();
                        if !clean.is_empty() {
                            return clean;
                        }
                    }
                }
            }

            // Fallback with sound ID
            return format!("Sound {}", sound_id);
        }

        "Unknown Sound".to_string()
    }

    /// Determines the sound category for a freesound URL using keyword matching.
    ///
    /// Checks keywords against:
    /// 1. The display name of the sound
    /// 2. The cached filename (which often contains descriptive words)
    ///
    /// Falls back to "freesound" if no keyword match is found.
    fn categorize_sound_by_keywords(
        url: &str,
        display_name: &str,
        cache_dir: &std::path::Path,
    ) -> String {
        // Collect all text to search through (lowercased, split into words)
        let mut search_words: Vec<String> = Vec::new();

        // Add display name words
        for word in display_name.to_lowercase().split_whitespace() {
            search_words.push(word.to_string());
        }

        // Add words from cached filename if available
        if let Some((creator, sound_id)) = parse_freesound_url(url) {
            let prefix = format!("{}_{}_", creator, sound_id);
            if let Ok(entries) = std::fs::read_dir(cache_dir) {
                for entry in entries.flatten() {
                    let filename = entry.file_name().to_string_lossy().to_string();
                    if filename.starts_with(&prefix) {
                        // Extract name part and split into words
                        let name_part = &filename[prefix.len()..];
                        let name_no_ext = name_part
                            .rsplit_once('.')
                            .map(|(n, _)| n)
                            .unwrap_or(name_part);
                        for word in name_no_ext.to_lowercase().replace('_', " ").split_whitespace()
                        {
                            if !search_words.contains(&word.to_string()) {
                                search_words.push(word.to_string());
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Match against keyword mappings
        for word in &search_words {
            for &(category, keywords) in SOUND_CATEGORY_KEYWORDS {
                if keywords.contains(&word.as_str()) {
                    return category.to_string();
                }
            }
        }

        // Default category
        "freesound".to_string()
    }

    /// Finds the project root by searching for the `env_conf/` directory.
    ///
    /// Search order:
    /// 1. Current working directory (preferred for desktop dev — uses the
    ///    real project tree where freesound.org/ cache already lives)
    /// 2. Parent directories (up to 3 levels above cwd)
    /// 3. Tauri resource directory (`${exe_dir}/assets` on desktop,
    ///    same on iOS via `BaseDirectory::Resource`)
    /// 4. iOS app bundle root -- on iOS, `resource_dir` resolves to
    ///    `${bundle}/assets` but files added via Xcode "Copy Bundle
    ///    Resources" are placed in the bundle root (`${bundle}/`).
    /// 5. iOS executable directory -- `std::env::current_exe()` parent,
    ///    which is the `.app` bundle on iOS.
    /// 6. Fallback to resource dir or cwd
    fn find_project_root(resource_dir: Option<PathBuf>) -> PathBuf {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // First, check CWD and parent directories. This is the common case
        // for desktop development where the user runs from the project root.
        // Checking CWD first ensures we use the real project tree (with
        // existing freesound.org/ cache) rather than the Tauri resource dir
        // (which contains bundled copies of env_conf/ but is a build artifact).
        if cwd.join("env_conf").exists() {
            tracing::info!("Project root found at CWD: {:?}", cwd);
            return cwd;
        }

        // Check parent directories (up to 3 levels)
        let mut dir = cwd.clone();
        for _ in 0..3 {
            if let Some(parent) = dir.parent() {
                dir = parent.to_path_buf();
                if dir.join("env_conf").exists() {
                    tracing::info!("Project root found at: {:?}", dir);
                    return dir;
                }
            }
        }

        // Check the Tauri resource directory (used on bundled desktop builds
        // and iOS where there is no project tree).
        if let Some(ref res_dir) = resource_dir {
            if res_dir.join("env_conf").exists() {
                tracing::info!("Project root found in Tauri resource dir: {:?}", res_dir);
                return res_dir.clone();
            }
            tracing::debug!("Tauri resource dir {:?} does not contain env_conf/", res_dir);

            // On iOS, resource_dir resolves to ${app_bundle}/assets but config
            // files injected via Xcode "Copy Bundle Resources" land in the
            // app bundle root (the parent directory). Check there too.
            if let Some(bundle_root) = res_dir.parent() {
                if bundle_root.join("env_conf").exists() {
                    tracing::info!(
                        "Project root found in iOS bundle root: {:?}",
                        bundle_root
                    );
                    return bundle_root.to_path_buf();
                }
                tracing::debug!(
                    "iOS bundle root {:?} does not contain env_conf/",
                    bundle_root
                );
            }
        }

        // On iOS, the executable lives inside the .app bundle. Files added
        // via "Copy Bundle Resources" are siblings of the executable. Check
        // the executable's parent directory.
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                if exe_dir.join("env_conf").exists() {
                    tracing::info!(
                        "Project root found via executable dir: {:?}",
                        exe_dir
                    );
                    return exe_dir.to_path_buf();
                }
                tracing::debug!(
                    "Executable dir {:?} does not contain env_conf/",
                    exe_dir
                );
            }
        }

        // On iOS, the resource dir may be the only valid location even if
        // env_conf doesn't exist yet (e.g., first run). Prefer it over cwd.
        if let Some(res_dir) = resource_dir {
            tracing::warn!(
                "Could not find env_conf/ directory, using Tauri resource dir: {:?}",
                res_dir
            );
            return res_dir;
        }

        // Fallback to cwd
        tracing::warn!("Could not find env_conf/ directory, using {:?}", cwd);
        cwd
    }

    /// Starts an environment.
    async fn start_environment(&mut self, config: &EnvironmentConfig) {
        tracing::info!("Starting environment: {} (category: {})", config.name, config.category);

        // Check if this is a sound-only config (one-shot sound effect)
        // Sound-only = has sound enabled, not a loop, and no lights/spotify/atmosphere
        let has_sound = config.engines.sound.as_ref()
            .map(|s| s.enabled && !s.is_loop)
            .unwrap_or(false);
        let has_lights = config.engines.lights.as_ref()
            .map(|l| l.enabled)
            .unwrap_or(false);
        let has_spotify = config.engines.spotify.as_ref()
            .map(|s| s.enabled && !s.context_uri.is_empty())
            .unwrap_or(false);
        let has_atmosphere = config.engines.atmosphere.as_ref()
            .map(|a| a.enabled)
            .unwrap_or(false);

        let is_sound_only = has_sound && !has_lights && !has_spotify && !has_atmosphere;

        if is_sound_only {
            // Sound-only config: just play the sound at 80% volume, don't stop anything else
            if let Some(ref sound) = config.engines.sound {
                tracing::info!("Playing one-shot sound effect: {} (sound-only config)", sound.file);
                self.active_sound_name = Some(Self::get_sound_display_name(&sound.file));
                if let Err(e) = self.sound_engine.play_async_with_volume(&sound.file, 80) {
                    tracing::warn!("Failed to play sound: {}", e);
                }
            }
            return;
        }

        // Full environment: stop existing atmosphere EXCEPT user-toggled loop sounds.
        // This preserves loop sounds the user explicitly started via toggle_loop_sound,
        // while clearing environment-specific atmosphere sounds.
        let old_atmosphere_count = self.active_atmosphere_urls.len();
        tracing::info!(
            "Stopping atmosphere (had {} tracked sounds, preserving {} loop sounds)",
            old_atmosphere_count, self.active_loop_urls.len()
        );
        self.atmosphere_engine.stop_all_except(&self.active_loop_urls);
        self.active_atmosphere_urls.retain(|url| self.active_loop_urls.contains(url));
        self.atmosphere_volumes.retain(|url, _| self.active_loop_urls.contains(url));

        // Reset pause state - new environment starts fresh
        self.sounds_paused = false;

        // Play entry sound
        if let Some(ref sound) = config.engines.sound {
            if sound.enabled && !sound.is_loop {
                tracing::info!("Playing sound: {}", sound.file);
                self.active_sound_name = Some(Self::get_sound_display_name(&sound.file));
                if let Err(e) = self.sound_engine.play_async_with_volume(&sound.file, 80) {
                    tracing::warn!("Failed to play sound: {}", e);
                }
            }
        }

        // Handle Spotify
        let has_spotify = config.engines.spotify.as_ref()
            .map(|s| s.enabled && !s.context_uri.is_empty())
            .unwrap_or(false);

        if has_spotify {
            // Start new Spotify playlist
            let spotify = config.engines.spotify.as_ref().unwrap();
            if let Some(ref engine) = self.spotify_engine {
                let engine = Arc::clone(engine);
                let uri = spotify.context_uri.clone();
                tokio::spawn(async move {
                    let engine = engine.lock().await;
                    // Ensure we're authenticated before playing
                    if let Err(e) = engine.authenticate().await {
                        tracing::warn!("Failed to authenticate Spotify: {}", e);
                        return;
                    }
                    if let Err(e) = engine.play_context(&uri).await {
                        tracing::warn!("Failed to start Spotify: {}", e);
                    }
                });
            }
        } else {
            // New environment has no Spotify - pause current playback
            if let Some(ref engine) = self.spotify_engine {
                let engine = Arc::clone(engine);
                tokio::spawn(async move {
                    let engine = engine.lock().await;
                    // Authenticate first in case token needs refresh
                    if let Err(e) = engine.authenticate().await {
                        tracing::warn!("Failed to authenticate for pause: {}", e);
                        return;
                    }
                    if let Err(e) = engine.pause().await {
                        tracing::warn!("Failed to pause Spotify: {}", e);
                    } else {
                        tracing::info!("Spotify paused (no Spotify in new environment)");
                    }
                });
            }
        }

        // Start atmosphere
        if let Some(ref atmosphere) = config.engines.atmosphere {
            if atmosphere.enabled {
                // Start new atmosphere mix — mark names for refresh after downloads finish
                self.needs_name_refresh = true;
                tracing::info!("Starting {} atmosphere sounds for {}", atmosphere.mix.len(), config.name);
                for sound in &atmosphere.mix {
                    let max_duration = sound.max_duration;
                    let fade_duration = sound.fade_duration;
                    tracing::info!(
                        "Starting atmosphere sound: {} at volume {}{}{}",
                        sound.url,
                        sound.volume,
                        max_duration.map_or(String::new(), |d| format!(" (max {}s)", d)),
                        fade_duration.map_or(String::new(), |d| format!(" (fade {}s)", d))
                    );
                    if let Err(e) = self
                        .atmosphere_engine
                        .start_single_with_options(&sound.url, sound.volume, fade_duration, max_duration)
                    {
                        tracing::warn!("Failed to start atmosphere sound: {}", e);
                    } else {
                        self.active_atmosphere_urls.insert(sound.url.clone());
                        self.atmosphere_volumes.insert(sound.url.clone(), sound.volume);
                    }
                }
            }
        } else {
            tracing::info!("No atmosphere config for {}", config.name);
        }

        // Update active lights config - always update when starting a new environment
        // This ensures the status bar reflects the current environment
        let old_lights_config = self.active_lights_config.clone();

        // Start lights
        if let Some(ref lights) = config.engines.lights {
            if lights.enabled && !self.lights_disabled_this_session {
                if let Some(ref anim_config) = lights.animation {
                    if let Some(ref engine) = self.lights_engine {
                        let engine = Arc::clone(engine);
                        let anim_config = anim_config.clone();
                        tokio::spawn(async move {
                            let mut engine = engine.lock().await;
                            if let Err(e) = engine.start(anim_config).await {
                                tracing::warn!("Failed to start lights: {}", e);
                            }
                        });
                        self.active_lights_config = Some(config.name.clone());
                        tracing::info!("Updated active_lights_config: {:?} -> {:?}", old_lights_config, self.active_lights_config);
                    }
                }
            }
        } else {
            // New environment has no lights - clear the active lights config
            // This prevents the status bar from showing the old environment
            if self.active_lights_config.is_some() {
                tracing::info!("Clearing active_lights_config (new env has no lights): {:?} -> None", old_lights_config);
                self.active_lights_config = None;
            }
        }
    }

    /// Stops all lights.
    async fn stop_lights(&mut self) {
        if let Some(ref engine) = self.lights_engine {
            let engine = Arc::clone(engine);
            tokio::spawn(async move {
                let mut engine = engine.lock().await;
                if let Err(e) = engine.stop().await {
                    tracing::warn!("Failed to stop lights: {}", e);
                }
                // Set warm white
                if let Err(e) = engine.set_warm_white().await {
                    tracing::warn!("Failed to set warm white: {}", e);
                }
            });
        }
        self.active_lights_config = None;
    }

    /// Stops atmosphere and Spotify.
    async fn stop_atmosphere(&mut self) {
        // Stop ALL atmosphere sounds including user-toggled loops
        self.atmosphere_engine.stop_all();
        self.active_atmosphere_urls.clear();
        self.atmosphere_volumes.clear();
        self.active_loop_urls.clear();

        // Pause Spotify - await directly instead of spawning
        if let Some(ref engine) = self.spotify_engine {
            tracing::info!("Pausing Spotify from stop_atmosphere...");
            let engine = engine.lock().await;
            // Authenticate first in case token needs refresh
            if let Err(e) = engine.authenticate().await {
                tracing::warn!("Failed to authenticate for pause: {}", e);
                return;
            }
            match engine.pause().await {
                Ok(()) => tracing::info!("Spotify paused successfully"),
                Err(e) => tracing::warn!("Failed to pause Spotify: {}", e),
            }
        }
    }

    /// Toggles a loop sound on/off by URL.
    fn toggle_loop_sound_url(&mut self, url: &str) -> Result<bool, String> {
        if url.is_empty() {
            return Err("Empty URL".to_string());
        }

        tracing::info!("Toggling loop sound: {}", url);

        if self.active_atmosphere_urls.contains(url) {
            // Sound is playing - stop it
            if let Err(e) = self.atmosphere_engine.stop_single(url) {
                tracing::warn!("Failed to stop loop sound: {}", e);
            }
            self.active_atmosphere_urls.remove(url);
            self.atmosphere_volumes.remove(url);
            self.active_loop_urls.remove(url);
            tracing::info!("Stopped loop sound: {}", url);
            Ok(false)
        } else {
            // Sound not playing - start it
            let volume = self.atmosphere_volumes.get(url).copied().unwrap_or(70);
            if let Err(e) = self.atmosphere_engine.start_single(url, volume) {
                return Err(format!("Failed to start loop sound: {}", e));
            }
            self.active_atmosphere_urls.insert(url.to_string());
            self.atmosphere_volumes.insert(url.to_string(), volume);
            self.active_loop_urls.insert(url.to_string());
            tracing::info!("Started loop sound: {}", url);
            Ok(true)
        }
    }

    /// Sets the volume for a URL.
    fn set_volume(&mut self, url: &str, volume: u8) {
        let volume = volume.clamp(10, 100);
        self.atmosphere_volumes.insert(url.to_string(), volume);

        // Update volume if sound is currently playing
        if self.active_atmosphere_urls.contains(url) {
            if let Err(e) = self.atmosphere_engine.set_volume(url, volume) {
                tracing::warn!("Failed to set volume: {}", e);
            }
        }
    }

    /// Searches configs across all categories.
    fn search_configs(&mut self, query: &str) {
        self.search_query = query.to_string();

        if query.is_empty() {
            self.search_results.clear();
            return;
        }

        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for configs in self.configs_by_category.values() {
            for config in configs {
                if self.config_matches_query(config, &query_lower) {
                    results.push(config.clone());
                }
            }
        }

        // Sort by relevance (name match first, then others)
        results.sort_by(|a, b| {
            let a_name_match = a.name.to_lowercase().contains(&query_lower);
            let b_name_match = b.name.to_lowercase().contains(&query_lower);
            b_name_match.cmp(&a_name_match).then(a.name.cmp(&b.name))
        });

        self.search_results = results;
    }

    /// Checks if a config matches a search query.
    fn config_matches_query(&self, config: &EnvironmentConfig, query: &str) -> bool {
        // Check name
        if config.name.to_lowercase().contains(query) {
            return true;
        }

        // Check category
        if config.category.to_lowercase().contains(query) {
            return true;
        }

        // Check description
        if let Some(ref desc) = config.description {
            if desc.to_lowercase().contains(query) {
                return true;
            }
        }

        // Check icon
        if let Some(ref icon) = config.icon {
            if icon.contains(query) {
                return true;
            }
        }

        // Check metadata tags
        if let Some(ref metadata) = config.metadata {
            for tag in &metadata.tags {
                if tag.to_lowercase().contains(query) {
                    return true;
                }
            }

            if let Some(ref intensity) = metadata.intensity {
                if intensity.to_lowercase().contains(query) {
                    return true;
                }
            }

            for suitable in &metadata.suitable_for {
                if suitable.to_lowercase().contains(query) {
                    return true;
                }
            }
        }

        false
    }

    /// Cleans up on exit.
    async fn cleanup(&mut self) {
        tracing::info!("Cleaning up...");

        // Stop all sounds
        self.sound_engine.stop_all();

        // Stop atmosphere
        self.atmosphere_engine.stop_all();

        // Stop Spotify
        if let Some(ref engine) = self.spotify_engine {
            tracing::info!("Pausing Spotify...");
            let engine = engine.lock().await;
            // Authenticate first in case token needs refresh
            if let Err(e) = engine.authenticate().await {
                tracing::warn!("Failed to authenticate for pause: {}", e);
            }
            match engine.pause().await {
                Ok(()) => tracing::info!("Spotify paused successfully"),
                Err(e) => tracing::warn!("Failed to pause Spotify: {}", e),
            }
        }

        // Stop lights animation and set to warm white - await this to ensure it completes before exit
        if let Some(ref engine) = self.lights_engine {
            tracing::info!("Stopping lights and setting warm white...");
            let mut engine = engine.lock().await;
            if let Err(e) = engine.stop().await {
                tracing::error!("Error stopping lights: {}", e);
            }
            if let Err(e) = engine.set_warm_white().await {
                tracing::error!("Error setting warm white: {}", e);
            }
            tracing::info!("Lights set to warm white");
        }
    }

    /// Gets available time variants for a config.
    fn get_available_times(&self, config_name: &str) -> AvailableTimes {
        // Try to find the config's source_path first (works across multiple directories)
        let times = if let Some(source_path) = self.find_config_source_path(config_name) {
            get_available_times_at_path(&source_path)
        } else {
            // Fallback: try primary config dir with filename heuristic
            let config_dir = self.config_loader.config_dir();
            let base_filename = format!("{}.yaml", config_name.to_lowercase().replace(' ', "_"));
            get_available_times_at_path(&config_dir.join(&base_filename))
        };
        let has_variants = !times.is_empty();

        AvailableTimes {
            config_name: config_name.to_string(),
            times,
            has_variants,
        }
    }

    /// Finds the source_path for a config by name (searches all loaded configs).
    fn find_config_source_path(&self, config_name: &str) -> Option<PathBuf> {
        self.configs_by_category
            .values()
            .flatten()
            .find(|c| c.name == config_name)
            .and_then(|c| c.source_path.clone())
    }

    /// Starts an environment with a specific time variant.
    /// Time variants are stored inline in the config's time_variants section.
    /// If time is "daytime", uses the base config (engines at root level).
    /// Otherwise, uses the engines from time_variants[time].
    async fn start_environment_with_time(
        &mut self,
        config_name: &str,
        time: &str,
    ) -> Result<(), String> {
        tracing::info!("start_environment_with_time called: config_name='{}', time='{}'", config_name, time);

        // Find the base config first
        let base_config = self
            .configs_by_category
            .values()
            .flatten()
            .find(|c| c.name == config_name)
            .cloned();

        if base_config.is_none() {
            tracing::error!("Config not found: '{}'", config_name);
            return Err(format!("Config not found: {}", config_name));
        }

        let base_config = base_config.unwrap();
        tracing::info!("Found base config: name='{}', category='{}', has_atmosphere={}, has_lights={}",
            base_config.name,
            base_config.category,
            base_config.engines.atmosphere.as_ref().map_or(false, |a| a.enabled),
            base_config.engines.lights.as_ref().map_or(false, |l| l.enabled)
        );

        // Use source_path if available, fallback to primary config_dir heuristic
        let config_path = base_config.source_path.clone().unwrap_or_else(|| {
            let config_dir = self.config_loader.config_dir();
            let base_filename = format!("{}.yaml", config_name.to_lowercase().replace(' ', "_"));
            config_dir.join(&base_filename)
        });
        tracing::debug!("Looking for time variants in: {:?}", config_path);

        // Update current time
        self.current_time = TimeOfDay::from_str(time).unwrap_or_default();

        // If time is "daytime", use base config directly (no variant overrides)
        if time == "daytime" {
            tracing::info!("Starting environment: {} (daytime/base)", config_name);
            self.start_environment(&base_config).await;
            return Ok(());
        }

        // Check for inline time variant
        let has_variants = has_time_variants_at_path(&config_path);
        tracing::debug!("has_time_variants({:?}) = {}", config_path, has_variants);

        if has_variants {
            if let Some(variant_engines) =
                get_time_variant_engines_at_path(&config_path, time)
            {
                // Create a modified config with the variant's engines
                // We need to parse the variant engines and apply them
                tracing::info!("Starting time variant: {} ({})", config_name, time);

                // Try to parse the variant engines into our types
                match serde_yaml::from_value::<immerse_core::config::EnginesConfig>(variant_engines)
                {
                    Ok(engines) => {
                        let mut variant_config = base_config.clone();
                        variant_config.engines = engines;
                        tracing::info!("Parsed variant engines: has_atmosphere={}, has_lights={}",
                            variant_config.engines.atmosphere.as_ref().map_or(false, |a| a.enabled),
                            variant_config.engines.lights.as_ref().map_or(false, |l| l.enabled)
                        );
                        self.start_environment(&variant_config).await;
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse time variant engines for {} ({}): {}",
                            config_name,
                            time,
                            e
                        );
                        // Fall through to start base config
                    }
                }
            } else {
                tracing::debug!("get_time_variant_engines returned None for {:?} ({})", config_path, time);
            }
        }

        // No variant found or failed to parse, start base config
        tracing::info!(
            "No time variant found for {} ({}), using base config",
            config_name,
            time
        );
        self.start_environment(&base_config).await;
        Ok(())
    }

    /// Gets categories that are sound categories (based on predefined list).
    /// Returns categories that exist in configs_by_category AND are in SOUND_CATEGORIES.
    fn get_sound_categories(&self) -> Vec<String> {
        SOUND_CATEGORIES
            .iter()
            .filter(|cat| self.configs_by_category.contains_key(**cat))
            .map(|s| s.to_string())
            .collect()
    }

    /// Checks if a category is a sound category (as opposed to environment category).
    fn is_sound_category(&self, category: &str) -> bool {
        SOUND_CATEGORIES.contains(&category)
    }

    /// Gets categories in proper order: environment categories first (in predefined order),
    /// then sound categories (in predefined order), then any other categories alphabetically.
    /// Hidden categories are excluded from the result.
    fn get_ordered_categories(&self) -> Vec<String> {
        let mut result = Vec::new();

        // First: environment categories in predefined order
        for cat in ENVIRONMENT_CATEGORIES {
            if self.configs_by_category.contains_key(*cat) {
                result.push(cat.to_string());
            }
        }

        // Second: sound categories in predefined order
        for cat in SOUND_CATEGORIES {
            if self.configs_by_category.contains_key(*cat) {
                result.push(cat.to_string());
            }
        }

        // Third: any other categories not in predefined lists (alphabetically)
        // Excludes hidden categories
        let predefined: HashSet<&str> = ENVIRONMENT_CATEGORIES
            .iter()
            .chain(SOUND_CATEGORIES.iter())
            .copied()
            .collect();
        let hidden: HashSet<&str> = HIDDEN_CATEGORIES.iter().copied().collect();
        let mut other: Vec<String> = self
            .configs_by_category
            .keys()
            .filter(|k| !predefined.contains(k.as_str()) && !hidden.contains(k.as_str()))
            .cloned()
            .collect();
        other.sort();
        result.extend(other);

        result
    }

    /// Sets the current time of day.
    fn set_current_time(&mut self, time: &str) -> Result<(), String> {
        self.current_time = TimeOfDay::from_str(time)
            .ok_or_else(|| format!("Invalid time: {}. Valid values: morning, daytime, afternoon, evening", time))?;
        Ok(())
    }


    /// Gets a clean display name for an atmosphere sound URL.
    /// Extracts the name from the cached filename and cleans it up:
    /// - Removes "freesound - " prefix (case insensitive)
    /// - Removes " by ..." suffix
    /// - Converts underscores to spaces
    fn get_atmosphere_display_name(&self, url: &str) -> String {
        // Try to find cached file and extract name from filename
        let cache_dir = &self.cache_dir;

        // Parse URL to get creator and sound_id
        let re = regex::Regex::new(r"freesound\.org/people/([^/]+)/sounds/(\d+)").ok();
        if let Some(re) = re {
            if let Some(caps) = re.captures(url) {
                let creator = &caps[1];
                let sound_id = &caps[2];
                let prefix = format!("{}_{}_", creator, sound_id);

                // Find matching cached file
                if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                    for entry in entries.flatten() {
                        let filename = entry.file_name().to_string_lossy().to_string();
                        if filename.starts_with(&prefix) {
                            // Extract name part (after creator_id_)
                            let name_with_ext = &filename[prefix.len()..];
                            // Remove extension
                            let name = name_with_ext
                                .rsplit_once('.')
                                .map(|(n, _)| n)
                                .unwrap_or(name_with_ext);

                            return Self::clean_display_name(name);
                        }
                    }
                }
            }
        }

        // Fallback: just return a shortened URL
        if let Some(sound_id) = url.split('/').filter(|s| !s.is_empty()).last() {
            format!("Sound {}", sound_id)
        } else {
            "Unknown".to_string()
        }
    }

    /// Gets a human-readable display name from a sound file reference.
    /// - `sound_conf:transition` → "Transition"
    /// - `sound_conf:squeaky_door` → "Squeaky Door"
    /// - `sounds/dooropen.wav` → "Dooropen"
    /// - `https://freesound.org/people/user/sounds/12345/` → "Sound 12345"
    fn get_sound_display_name(file: &str) -> String {
        // sound_conf reference: "sound_conf:transition" -> "Transition"
        if let Some(conf_name) = file.strip_prefix("sound_conf:") {
            return conf_name
                .replace('_', " ")
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
        }

        // Freesound URL: extract sound ID
        if file.contains("freesound.org") {
            if let Some((_creator, sound_id)) = immerse_core::download_queue::parse_freesound_url(file) {
                return format!("Sound {}", sound_id);
            }
        }

        // Local file: extract filename stem, replace underscores with spaces
        std::path::Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file)
            .replace('_', " ")
    }

    /// Cleans up a display name by removing common prefixes/suffixes.
    fn clean_display_name(name: &str) -> String {
        let mut result = name.replace('_', " ");

        // Remove "freesound - " prefix (case insensitive)
        let lower = result.to_lowercase();
        if lower.starts_with("freesound - ") {
            result = result[12..].to_string();
        } else if lower.starts_with("freesound-") {
            result = result[10..].to_string();
        }

        // Remove " by ..." suffix (find last occurrence of " by " and remove everything after)
        if let Some(idx) = result.to_lowercase().rfind(" by ") {
            result = result[..idx].to_string();
        }

        // Trim whitespace
        result.trim().to_string()
    }

    /// Gets a display name with author info for tooltips (keeps "by Author").
    fn get_atmosphere_name_with_author(&self, url: &str) -> String {
        // Try to find cached file and extract name from filename
        let cache_dir = &self.cache_dir;

        // Parse URL to get creator and sound_id
        let re = regex::Regex::new(r"freesound\.org/people/([^/]+)/sounds/(\d+)").ok();
        if let Some(re) = re {
            if let Some(caps) = re.captures(url) {
                let creator = &caps[1];
                let sound_id = &caps[2];
                let prefix = format!("{}_{}_", creator, sound_id);

                // Find matching cached file
                if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                    for entry in entries.flatten() {
                        let filename = entry.file_name().to_string_lossy().to_string();
                        if filename.starts_with(&prefix) {
                            // Extract name part (after creator_id_)
                            let name_with_ext = &filename[prefix.len()..];
                            // Remove extension
                            let name = name_with_ext
                                .rsplit_once('.')
                                .map(|(n, _)| n)
                                .unwrap_or(name_with_ext);

                            // Clean but KEEP the author
                            return Self::clean_display_name_keep_author(name);
                        }
                    }
                }
            }
        }

        // Fallback: just return a shortened URL
        if let Some(sound_id) = url.split('/').filter(|s| !s.is_empty()).last() {
            format!("Sound {}", sound_id)
        } else {
            "Unknown".to_string()
        }
    }

    /// Cleans up a display name but keeps the author info.
    fn clean_display_name_keep_author(name: &str) -> String {
        let mut result = name.replace('_', " ");

        // Remove "freesound - " prefix (case insensitive)
        let lower = result.to_lowercase();
        if lower.starts_with("freesound - ") {
            result = result[12..].to_string();
        } else if lower.starts_with("freesound-") {
            result = result[10..].to_string();
        }

        // Keep "by ..." for author attribution
        // Trim whitespace
        result.trim().to_string()
    }

    // ========================================================================
    // Settings Methods
    // ========================================================================

    /// Gets the current Spotify configuration.
    fn get_spotify_config(&self) -> SpotifyConfig {
        let config_path = self.project_root.join(".spotify.ini");
        let settings_path = self.project_root.join("settings.ini");

        let mut config = SpotifyConfig::default();
        config.redirect_uri = "http://127.0.0.1:8888/callback".to_string();
        config.auto_start = "ask".to_string();

        // Read Spotify credentials from .spotify.ini
        if config_path.exists() {
            let mut ini = configparser::ini::Ini::new();
            if ini.load(&config_path).is_ok() {
                config.username = ini.get("DEFAULT", "username").unwrap_or_default();
                config.client_id = ini.get("DEFAULT", "client_id").unwrap_or_default();
                config.client_secret = ini.get("DEFAULT", "client_secret").unwrap_or_default();
                if let Some(uri) = ini.get("DEFAULT", "redirectURI") {
                    if !uri.is_empty() {
                        config.redirect_uri = uri;
                    }
                }
                config.is_configured = !config.client_id.is_empty() && !config.client_secret.is_empty();
            }
        }

        // Read auto_start setting from settings.ini
        if settings_path.exists() {
            let mut ini = configparser::ini::Ini::new();
            if ini.load(&settings_path).is_ok() {
                if let Some(auto_start) = ini.get("spotify", "auto_start") {
                    if !auto_start.is_empty() {
                        config.auto_start = auto_start;
                    }
                }
            }
        }

        config
    }

    /// Saves the Spotify configuration.
    fn save_spotify_config(&self, config: SpotifyConfig) -> Result<(), String> {
        let config_path = self.project_root.join(".spotify.ini");
        let settings_path = self.project_root.join("settings.ini");

        // Save credentials to .spotify.ini
        let content = format!(
            "[DEFAULT]\nusername = {}\nclient_id = {}\nclient_secret = {}\nredirectURI = {}\n",
            config.username, config.client_id, config.client_secret, config.redirect_uri
        );
        std::fs::write(&config_path, content).map_err(|e| format!("Failed to save .spotify.ini: {}", e))?;

        // Save auto_start to settings.ini
        self.update_settings_ini(&settings_path, "spotify", "auto_start", &config.auto_start)?;

        tracing::info!("Saved Spotify configuration");
        Ok(())
    }

    /// Gets the current WIZ bulb configuration.
    fn get_wizbulb_config(&self) -> WizBulbConfig {
        let config_path = self.project_root.join(".wizbulb.ini");

        let mut config = WizBulbConfig::default();

        if config_path.exists() {
            let mut ini = configparser::ini::Ini::new();
            if ini.load(&config_path).is_ok() {
                config.backdrop_bulbs = ini.get("DEFAULT", "backdrop_bulbs").unwrap_or_default();
                config.overhead_bulbs = ini.get("DEFAULT", "overhead_bulbs").unwrap_or_default();
                config.battlefield_bulbs = ini.get("DEFAULT", "battlefield_bulbs").unwrap_or_default();
                config.is_configured = !config.backdrop_bulbs.is_empty()
                    || !config.overhead_bulbs.is_empty()
                    || !config.battlefield_bulbs.is_empty();
            }
        }

        config
    }

    /// Saves the WIZ bulb configuration.
    fn save_wizbulb_config(&self, config: WizBulbConfig) -> Result<(), String> {
        let config_path = self.project_root.join(".wizbulb.ini");

        let content = format!(
            "[DEFAULT]\nbackdrop_bulbs = {}\noverhead_bulbs = {}\nbattlefield_bulbs = {}\n",
            config.backdrop_bulbs, config.overhead_bulbs, config.battlefield_bulbs
        );
        std::fs::write(&config_path, content).map_err(|e| format!("Failed to save .wizbulb.ini: {}", e))?;

        tracing::info!("Saved WIZ bulb configuration");
        Ok(())
    }

    /// Gets the current app settings.
    fn get_app_settings(&self) -> AppSettings {
        let settings_path = self.project_root.join("settings.ini");

        let mut settings = AppSettings::default();

        if settings_path.exists() {
            let mut ini = configparser::ini::Ini::new();
            if ini.load(&settings_path).is_ok() {
                settings.ignore_ssl_errors = ini.get("downloads", "ignore_ssl_errors")
                    .map(|v| v.to_lowercase() == "true")
                    .unwrap_or(false);
                settings.spotify_auto_start = ini.get("spotify", "auto_start")
                    .unwrap_or_else(|| "ask".to_string());
            }
        }

        settings
    }

    /// Saves the app settings.
    fn save_app_settings(&self, settings: AppSettings) -> Result<(), String> {
        let settings_path = self.project_root.join("settings.ini");

        // Update both settings
        self.update_settings_ini(&settings_path, "downloads", "ignore_ssl_errors",
            if settings.ignore_ssl_errors { "true" } else { "false" })?;
        self.update_settings_ini(&settings_path, "spotify", "auto_start", &settings.spotify_auto_start)?;

        tracing::info!("Saved app settings");
        Ok(())
    }

    /// Helper to update a single setting in settings.ini.
    fn update_settings_ini(&self, path: &PathBuf, section: &str, key: &str, value: &str) -> Result<(), String> {
        let mut ini = configparser::ini::Ini::new();

        // Load existing content if file exists
        if path.exists() {
            let _ = ini.load(path);
        }

        // Set the value
        ini.set(section, key, Some(value.to_string()));

        // Write back
        ini.write(path).map_err(|e| format!("Failed to save settings.ini: {}", e))?;

        Ok(())
    }
}

/// Discovers WIZ bulbs on the network.
/// Uses UDP broadcast to find bulbs on the local network.
pub async fn discover_bulbs() -> Result<Vec<String>, String> {
    use std::net::UdpSocket;
    use std::time::Duration;

    tracing::info!("Starting WIZ bulb discovery...");

    // WIZ bulbs listen on port 38899 for UDP messages
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to create socket: {}", e))?;

    socket.set_broadcast(true)
        .map_err(|e| format!("Failed to enable broadcast: {}", e))?;

    socket.set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    // Send discovery message to broadcast address
    let discovery_msg = r#"{"method":"getPilot"}"#;
    let broadcast_addrs = ["192.168.1.255:38899", "192.168.0.255:38899", "255.255.255.255:38899"];

    for addr in &broadcast_addrs {
        if let Err(e) = socket.send_to(discovery_msg.as_bytes(), addr) {
            tracing::warn!("Failed to send to {}: {}", addr, e);
        }
    }

    // Collect responses
    let mut bulbs = Vec::new();
    let mut buf = [0u8; 1024];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, addr)) => {
                let ip = addr.ip().to_string();
                if !bulbs.contains(&ip) {
                    // Verify it's a WIZ bulb response
                    if let Ok(response) = std::str::from_utf8(&buf[..len]) {
                        if response.contains("result") || response.contains("method") {
                            tracing::info!("Found WIZ bulb at: {}", ip);
                            bulbs.push(ip);
                        }
                    }
                }
            }
            Err(_) => break, // Timeout or error, stop listening
        }
    }

    tracing::info!("Discovery complete. Found {} bulb(s)", bulbs.len());
    Ok(bulbs)
}
