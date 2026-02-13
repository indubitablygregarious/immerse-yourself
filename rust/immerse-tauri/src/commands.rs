//! Tauri commands for the frontend to call.

use std::collections::HashMap;

use immerse_core::config::EnvironmentConfig;
use tauri::State;

use crate::state::{ActiveState, AppState, AvailableTimes};

/// Gets all category names.
#[tauri::command]
pub fn get_categories(state: State<AppState>) -> Vec<String> {
    state.get_categories()
}

/// Gets environments for a specific category.
#[tauri::command]
pub fn get_environments(state: State<AppState>, category: &str) -> Vec<EnvironmentConfig> {
    state.get_environments(category)
}

/// Gets all configs across all categories.
#[tauri::command]
pub fn get_all_configs(state: State<AppState>) -> HashMap<String, Vec<EnvironmentConfig>> {
    state.get_all_configs()
}

/// Starts an environment by name.
#[tauri::command]
pub fn start_environment(state: State<AppState>, config_name: &str) -> Result<(), String> {
    state.start_environment(config_name)
}

/// Toggles a loop sound on/off.
/// Returns true if the sound is now playing, false if stopped.
#[tauri::command]
pub fn toggle_loop_sound(state: State<AppState>, url: &str) -> Result<bool, String> {
    state.toggle_loop_sound(url)
}

/// Sets the volume for a URL.
#[tauri::command]
pub fn set_volume(state: State<AppState>, url: &str, volume: u8) -> Result<(), String> {
    state.set_volume(url, volume)
}

/// Stops all lights.
#[tauri::command]
pub fn stop_lights(state: State<AppState>) -> Result<(), String> {
    state.stop_lights()
}

/// Stops all sounds.
/// Returns the number of sounds that were stopped.
#[tauri::command]
pub fn stop_sounds(state: State<AppState>) -> usize {
    state.stop_sounds()
}

/// Stops atmosphere and Spotify.
#[tauri::command]
pub fn stop_atmosphere(state: State<AppState>) -> Result<(), String> {
    state.stop_atmosphere()
}

/// Toggles pause/resume on all sounds (both sound engine and atmosphere).
/// Returns true if sounds are now paused, false if resumed.
#[tauri::command]
pub fn toggle_pause_sounds(state: State<AppState>) -> bool {
    state.toggle_pause_sounds()
}

/// Clears the freesound download cache.
/// Stops all playing sounds, then deletes all cached files.
/// Returns the number of files deleted.
#[tauri::command]
pub fn clear_sound_cache(state: State<AppState>) -> Result<usize, String> {
    state.clear_sound_cache()
}

/// Reloads all YAML configs from disk and regenerates virtual loop configs.
/// Returns the total number of configs loaded.
#[tauri::command]
pub fn reload_configs(state: State<AppState>) -> Result<usize, String> {
    state.reload_configs()
}

/// Searches configs across all categories.
#[tauri::command]
pub fn search_configs(state: State<AppState>, query: &str) -> Vec<EnvironmentConfig> {
    state.search_configs(query)
}

/// Gets the current active state.
#[tauri::command]
pub fn get_active_state(state: State<AppState>) -> ActiveState {
    state.get_active_state()
}

/// Gets available time variants for a config.
#[tauri::command]
pub fn get_available_times(state: State<AppState>, config_name: &str) -> AvailableTimes {
    state.get_available_times(config_name)
}

/// Starts an environment with a specific time variant.
#[tauri::command]
pub fn start_environment_with_time(
    state: State<AppState>,
    config_name: &str,
    time: &str,
) -> Result<(), String> {
    state.start_environment_with_time(config_name, time)
}

/// Gets categories that are sound-only (all configs in category are loop sounds).
#[tauri::command]
pub fn get_sound_categories(state: State<AppState>) -> Vec<String> {
    state.get_sound_categories()
}

/// Sets the current time of day for future environment starts.
#[tauri::command]
pub fn set_current_time(state: State<AppState>, time: &str) -> Result<(), String> {
    state.set_current_time(time)
}

/// Triggers the startup environment (hidden "Startup" config).
/// Called when the app opens to initialize the ambient environment.
/// Returns the name of the environment that was started, or None if not found.
#[tauri::command]
pub fn trigger_startup(state: State<AppState>) -> Option<String> {
    state.trigger_startup_environment()
}

/// Returns the user content directory path, or None if not configured.
#[tauri::command]
pub fn get_user_content_dir(state: State<AppState>) -> Option<String> {
    state.get_user_content_dir()
}

// ============================================================================
// Settings Commands
// ============================================================================

/// Gets the current Spotify configuration.
#[tauri::command]
pub fn get_spotify_config(state: State<AppState>) -> crate::state::SpotifyConfig {
    state.get_spotify_config()
}

/// Saves the Spotify configuration.
#[tauri::command]
pub fn save_spotify_config(state: State<AppState>, config: crate::state::SpotifyConfig) -> Result<(), String> {
    state.save_spotify_config(config)
}

/// Gets the current WIZ bulb configuration.
#[tauri::command]
pub fn get_wizbulb_config(state: State<AppState>) -> crate::state::WizBulbConfig {
    state.get_wizbulb_config()
}

/// Saves the WIZ bulb configuration.
#[tauri::command]
pub fn save_wizbulb_config(state: State<AppState>, config: crate::state::WizBulbConfig) -> Result<(), String> {
    state.save_wizbulb_config(config)
}

/// Gets the current app settings.
#[tauri::command]
pub fn get_app_settings(state: State<AppState>) -> crate::state::AppSettings {
    state.get_app_settings()
}

/// Saves the app settings.
#[tauri::command]
pub fn save_app_settings(state: State<AppState>, settings: crate::state::AppSettings) -> Result<(), String> {
    state.save_app_settings(settings)
}

/// Discovers WIZ bulbs on the network.
/// Returns a list of discovered bulb IP addresses.
#[tauri::command]
pub async fn discover_bulbs() -> Result<Vec<String>, String> {
    crate::state::discover_bulbs().await
}

// ============================================================================
// Debug Log Commands
// ============================================================================

/// Returns the most recent debug log entries from the in-memory buffer.
#[tauri::command]
pub fn get_debug_log(state: State<AppState>) -> Vec<String> {
    state.get_debug_log()
}

/// Clears the in-memory debug log buffer.
#[tauri::command]
pub fn clear_debug_log(state: State<AppState>) {
    state.clear_debug_log()
}
