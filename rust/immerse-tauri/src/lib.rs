//! Tauri library for Immerse Yourself.

mod commands;
mod state;

pub use commands::*;
pub use state::*;

#[cfg(desktop)]
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
#[allow(unused_imports)]
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri::path::BaseDirectory;

use std::collections::VecDeque;
use std::sync::Arc;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

// ============================================================================
// In-memory log buffer layer for tracing
// ============================================================================

/// A tracing layer that captures log events into a shared in-memory ring buffer.
/// This allows the iOS debug UI to display backend logs without stdout access.
struct BufferLayer {
    buffer: LogBuffer,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for BufferLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // Extract the message text
        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);

        // Format: [HH:MM:SS] LEVEL message
        let now = chrono_lite_now();
        let level = event.metadata().level();
        let line = format!("[{}] {:>5} {}", now, level, visitor.0);

        if let Ok(mut buf) = self.buffer.lock() {
            buf.push_back(line);
            while buf.len() > 500 {
                buf.pop_front();
            }
        }
    }
}

/// Visitor that extracts the `message` field from a tracing event.
struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = value.to_string();
        }
    }
}

/// Returns current time as HH:MM:SS without pulling in the chrono crate.
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Runs the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Shared log buffer for in-app debug viewer
    let log_buffer: LogBuffer = Arc::new(std::sync::Mutex::new(VecDeque::new()));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        );

    let buffer_layer = BufferLayer {
        buffer: Arc::clone(&log_buffer),
    };

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(buffer_layer)
        .init();

    let log_buffer_for_setup = Arc::clone(&log_buffer);

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(move |app| {
            // Resolve the Tauri resource directory for bundled config files.
            // On iOS/mobile, this is where env_conf/ and sound_conf/ are placed.
            // On desktop during development, this may not contain configs (they
            // are found via CWD traversal instead).
            let resource_dir = app
                .path()
                .resolve("", BaseDirectory::Resource)
                .ok();
            tracing::info!("Tauri resource dir: {:?}", resource_dir);

            // On iOS the project root is inside the read-only app bundle, so
            // freesound downloads must go to a writable location.  Tauri's
            // app_cache_dir() gives us a per-app writable directory.
            let cache_dir = app.path().app_cache_dir().ok().map(|d| d.join("freesound.org"));
            tracing::info!("Writable cache dir: {:?}", cache_dir);

            // Platform-standard app data directory for user content
            // Linux: ~/.local/share/com.peterlesko.immerseyourself/
            // macOS: ~/Library/Application Support/com.peterlesko.immerseyourself/
            let user_content_dir = app.path().app_data_dir().ok();
            tracing::info!("User content dir: {:?}", user_content_dir);

            let state = AppState::new_with_resource_dir_and_log(
                resource_dir,
                log_buffer_for_setup.clone(),
                cache_dir,
                user_content_dir,
            );
            app.manage(state);

            #[cfg(desktop)]
            {
                // Create Settings menu item (no accelerator - handled by JS)
                let settings_item = MenuItemBuilder::with_id("settings", "Settings")
                    .build(app)?;

                // Create Quit menu item with Ctrl+Q accelerator
                let quit_item = MenuItemBuilder::with_id("quit", "Quit")
                    .accelerator("Ctrl+Q")
                    .build(app)?;

                // Build the File submenu
                let file_menu = SubmenuBuilder::new(app, "File")
                    .item(&settings_item)
                    .separator()
                    .item(&quit_item)
                    .build()?;

                // Build the main menu bar
                let menu = MenuBuilder::new(app).item(&file_menu).build()?;

                // Set the menu on the app
                app.set_menu(menu)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_categories,
            commands::get_environments,
            commands::get_all_configs,
            commands::start_environment,
            commands::toggle_loop_sound,
            commands::set_volume,
            commands::stop_lights,
            commands::stop_sounds,
            commands::stop_atmosphere,
            commands::toggle_pause_sounds,
            commands::clear_sound_cache,
            commands::reload_configs,
            commands::search_configs,
            commands::get_active_state,
            commands::get_available_times,
            commands::start_environment_with_time,
            commands::get_sound_categories,
            commands::set_current_time,
            commands::trigger_startup,
            // User content
            commands::get_user_content_dir,
            // Settings commands
            commands::get_spotify_config,
            commands::save_spotify_config,
            commands::get_wizbulb_config,
            commands::save_wizbulb_config,
            commands::get_app_settings,
            commands::save_app_settings,
            commands::discover_bulbs,
            // Debug log commands
            commands::get_debug_log,
            commands::clear_debug_log,
        ]);

    // Menu events are only available on desktop
    #[cfg(desktop)]
    let builder = builder.on_menu_event(|app, event| {
        match event.id().as_ref() {
            "settings" => {
                tracing::info!("Settings menu item clicked");
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(e) = window.eval("window.__openSettings && window.__openSettings()") {
                        tracing::error!("Failed to eval settings opener: {}", e);
                    }
                }
            }
            "quit" => {
                if let Some(state) = app.try_state::<AppState>() {
                    tracing::info!("Quit requested from menu, cleaning up...");
                    state.cleanup();
                }
                app.exit(0);
            }
            _ => {}
        }
    });

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        match event {
            RunEvent::WindowEvent {
                event: WindowEvent::CloseRequested { .. },
                ..
            } => {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    tracing::info!("Window closing, cleaning up...");
                    state.cleanup();
                }
            }
            RunEvent::ExitRequested { .. } => {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    tracing::info!("Exit requested, cleaning up...");
                    state.cleanup();
                }
            }
            _ => {}
        }
    });
}
