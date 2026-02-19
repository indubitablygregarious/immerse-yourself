# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## ABSOLUTE RULE: No Secrets in Code or Tracked Files

NEVER write API tokens, keys, passwords, client secrets, or any sensitive credentials into ANY file unless that file is explicitly listed in `.gitignore`. Always read credentials from existing config files (`.spotify.ini`, `.wizbulb.ini`, `.freesound.ini`, etc.) at runtime. If a skill or script needs an API key, it must read it from the config file — never hardcode it.

## Project Overview

"Immerse Yourself" is an interactive ambient environment system for tabletop RPG sessions that combines:
- **Spotify music playback** -- Different playlists for various scenes/moods
- **Smart light control** -- WIZ smart bulbs with synchronized color and animation patterns
- **Sound effects** -- Local audio files that play when scenes are triggered
- **Atmosphere loops** -- Ambient sound mixes from freesound.org URLs

The application is built with **Tauri 2.x** (Rust backend + React TypeScript frontend).

## Directory Structure

```
rust/
  immerse-core/           # Shared Rust library (config loading, engines, FFI)
    src/
      config/             # YAML config loader, types, validator
      engines/            # Sound, Spotify, Lights, Atmosphere engines
      download_queue.rs   # Freesound.org download management
      ffi.rs              # Swift/iOS interop layer
  immerse-tauri/          # Tauri application
    src/
      lib.rs              # App init, native menu, command registration
      commands.rs         # Tauri IPC commands exposed to frontend
      state.rs            # AppState + AppStateInner (all application state)
    ui/                   # React TypeScript frontend (Vite)
      src/
        App.tsx           # Main component, keyboard shortcuts, dialog state
        components/       # UI components (13 .tsx files)
        hooks/useAppState.ts  # State management hook with Tauri IPC
        types/index.ts    # TypeScript interfaces matching Rust structs
        contexts/         # ThemeContext
        styles/           # CSS
env_conf/                 # Environment YAML configs (see env_conf/README.md)
sound_conf/               # Sound collection YAML configs (see sound_conf/README.md)
sounds/                   # Local sound effect files
freesound.org/            # Cached freesound downloads (not in git)
scripts/
  desktop-release.py      # Release automation (desktop + iOS, --ios-only flag)
devlog/                   # Development diary entries
tests/
  e2e/                    # E2E tests and screenshot automation
    tauri_app.py          # TauriApp helper (WebKit Inspector Protocol + keyboard fallback)
    run_local.py          # Local screenshot runner (no Docker needed)
    test_screenshots.py   # Screenshot test (Travel > Afternoon)
    Dockerfile            # Multi-stage Docker build for CI
    run.sh                # Container entrypoint (Xvfb, PulseAudio, D-Bus)
.venv/                    # Python virtual environment for ruff (not in git)
Makefile                  # Build commands (MUST use these, not cargo directly)
```

## Build System

**IMPORTANT**: Always use Makefile targets for building and testing. The system has Rust 1.75 as default but Tauri requires Rust 1.89. The Makefile creates a wrapper that uses the correct version.

**Never run `cargo build`, `cargo test`, or `cargo check` directly** -- it will fail with Rust version errors.

```bash
make setup      # Install Tauri CLI + configure git hooks (run once after clone)
make help       # Show all available commands
make dev        # Start Tauri dev server with hot reload (frontend + backend)
make dev-full   # Dev with full content from private content repo
make build      # Build production application (includes npm build)
make test       # Run Rust tests (uses Rust 1.89 wrapper)
make check      # Check code compiles without building (uses Rust 1.89 wrapper)
make run        # Run pre-built application (alias: make d, make desktop)
make trex       # Build and launch application
make clean      # Remove build artifacts
make release             # Cut a release — builds desktop + iOS (bump, tag, push, monitor)
make release-dry-run     # Preview what a release would do
make release-ios         # iOS TestFlight only (bump, push to main, no tag)
make release-ios-dry-run # Preview what an iOS-only release would do
make lint                # Lint Python scripts with ruff (requires .venv)
```

Use `RELEASE_ARGS` to pass options: `make release RELEASE_ARGS="--minor"`, `make release RELEASE_ARGS="--major"`, or `make release RELEASE_ARGS="--version 1.0.0"`.

### Releases (Desktop + iOS)

Both platforms read from the same version in `tauri.conf.json`. A single `make release` triggers both:
- The `v*` tag triggers `desktop-build.yml` (Linux, macOS, Windows binaries + GitHub Release)
- The push to `main` triggers `ios-build.yml` (TestFlight build)

For iOS-only iterations (no desktop release), use `make release-ios` — it pushes to main without a tag.

**Desktop workflow:** `.github/workflows/desktop-build.yml`
- Push to `main` — CI build (compile + test, no release)
- Push `v*` tag — full build + GitHub Release with platform binaries
- Platform jobs: Linux (tar.gz), macOS (DMG), Windows (zip)

**iOS workflow:** `.github/workflows/ios-build.yml`
- Triggers on push to `main` when `rust/immerse-tauri/tauri.conf.json` changes

**Full release flow** (`make release`):
1. Script bumps version in `tauri.conf.json`, commits, creates annotated `v*` tag, pushes
2. Tag triggers desktop build; push triggers iOS build
3. Script monitors the desktop build and reports the release URL when done

**iOS-only flow** (`make release-ios`):
1. Script bumps version, commits, pushes to main (no tag)
2. Only `ios-build.yml` triggers
3. Script monitors the iOS build

Script options: `--minor`, `--major`, `--version X.Y.Z`, `--dry-run`, `--no-monitor`, `--monitor-only vX.Y.Z`, `--ios-only`.

### Git Hooks

Pre-push hooks live in `.githooks/` (checked into the repo). `make setup` configures `core.hooksPath` to use them. Current hooks:

- **pre-push**: When workflow files in `.github/workflows/` are part of the push, runs Claude to validate for non-existent actions, macOS runner costs, missing caching, and missing concurrency guards. Prompts Y/n before proceeding.

### E2E Testing & Screenshots

```bash
make screenshot  # Capture project screenshot -- runs locally
make e2e-build   # Build E2E test Docker image
make e2e         # Run all E2E tests in Docker (builds image first)
```

### iOS Builds (macOS only)

```bash
make ios-setup   # Install prerequisites (Rust, Node, Tauri CLI, iOS targets)
make ios-init    # Initialize Xcode project (first time only)
make ios-dev     # Run on iOS Simulator with hot reload
make ios-build   # Build frontend + Rust for iOS (debug)
make ios-release # Build release IPA for TestFlight
make ios-sim     # Build and run on iOS Simulator
make ios-open    # Open Xcode project for manual signing/debugging
```

### Linting

Python scripts are linted with [ruff](https://docs.astral.sh/ruff/). The linter runs from a `.venv` virtual environment.

```bash
make lint                # Lint scripts/ and tests/e2e/ with ruff
```

To set up the venv (first time only):
```bash
python3 -m venv .venv && .venv/bin/pip3 install ruff
```

### Windows Smoke Test

```bash
make test-windows              # Trigger smoke test on GitHub Actions (real Windows)
make test-windows VERSION=TAG  # Test a specific release version
make test-windows-status       # Check smoke test run status
make test-windows-screenshot   # Download screenshot from latest completed run
```

### Development with Content

The public repo ships with empty `env_conf/`, `sound_conf/`, and `sounds/` directories. To develop with content:

1. **User content directory**: Place environment configs and sounds in your user content directory (auto-created on first launch):
   - **Linux**: `~/.local/share/com.peterlesko.immerseyourself/`
   - **macOS**: `~/Library/Application Support/com.peterlesko.immerseyourself/`

2. **`make dev-full`**: If you have a private content repo, set `IMMERSE_PRIVATE_REPO` and run `make dev-full` to copy content into the user content directory before launching.

## Architecture (Rust/Tauri)

### Backend (Rust)

**`state.rs` -- AppState and AppStateInner**

All mutable application state lives in `AppStateInner`, wrapped by `AppState` (thread-safe with `tokio::Mutex`). Key fields:

- `config_loader` / `configs_by_category` -- Loaded YAML environment configs
- `active_lights_config` -- Name of the currently active lights environment
- `active_sound_name` -- Name of currently playing one-shot sound
- `active_atmosphere_urls` / `atmosphere_volumes` -- Active atmosphere sounds and their volumes
- `current_time` -- Current time-of-day (morning/daytime/afternoon/evening)
- `sound_engine` / `lights_engine` / `atmosphere_engine` / `spotify_engine` -- Engine instances

`AppState` methods use `runtime.block_on()` to bridge sync Tauri commands to async engine operations. Pre-download logic releases the inner lock while waiting for freesound downloads, so `get_active_state` polling continues working.

**`commands.rs` -- Tauri IPC Commands**

Thin wrappers that delegate to `AppState` methods:

| Command | Description |
|---------|-------------|
| `get_categories` | Ordered category list (env first, then sounds) |
| `get_environments` | Configs for a category (filtered by env vs sound) |
| `get_all_configs` | All configs across all categories |
| `start_environment` | Start environment by name (with pre-download) |
| `start_environment_with_time` | Start with specific time variant |
| `toggle_loop_sound` | Toggle loop sound on/off by URL |
| `set_volume` | Set volume for atmosphere URL |
| `stop_lights` / `stop_sounds` / `stop_atmosphere` | Stop engines |
| `search_configs` | Fuzzy search across all config metadata |
| `get_active_state` | Snapshot of all active state (polled at 1s intervals) |
| `get_available_times` | Time variants for a config |
| `trigger_startup` | Auto-start "Startup" or "Travel" environment |
| `get_spotify_config` / `save_spotify_config` | Spotify settings |
| `get_wizbulb_config` / `save_wizbulb_config` | WIZ bulb settings |
| `get_app_settings` / `save_app_settings` | General app settings |
| `discover_bulbs` | UDP broadcast discovery of WIZ bulbs |
| `get_user_content_dir` | Path to user content directory |

**`lib.rs` -- App Initialization**

Sets up tracing, Tauri plugins, native File menu (Settings + Quit), registers all IPC commands, and handles cleanup on window close/exit (stops engines, sets lights to warm white).

**`immerse-core` -- Shared Library**

- `config/` -- `ConfigLoader`, `EnvironmentConfig`, `EnginesConfig`, time variant resolution
- `engines/` -- `SoundEngine` (ffplay/aplay subprocess), `AtmosphereEngine` (looping ffplay with PulseAudio volume), `LightsEngine` (WIZ bulb async control), `SpotifyEngine` (OAuth + playback)
- `download_queue.rs` -- Freesound URL parsing and download management
- `ffi.rs` -- C-compatible FFI for Swift/iOS interop

### Frontend (React TypeScript)

**`useAppState.ts` -- Central State Hook**

Loads initial data on mount (`get_categories`, `get_all_configs`, `get_active_state`, `get_sound_categories`), triggers startup environment, then polls `get_active_state` every 1 second. Exposes all IPC calls as React callbacks.

**`App.tsx` -- Main Component**

Handles keyboard shortcuts, responsive layout (mobile breakpoint at 960px), time variant dialog, settings dialog. Key keyboard shortcuts:
- `Q-L` -- Environment button shortcuts (remapped per category/search)
- `1-4` -- Time of day (only when time variants are available)
- `5` -- Navigate to active lights config
- `Ctrl+PgUp/PgDn` -- Category navigation
- `Ctrl+,` -- Settings dialog
- `Ctrl+Q` -- Quit
- `Escape` -- Clear search
- `Enter` -- First press focuses search result, second press activates

**Components** (`ui/src/components/`):

| Component | Purpose |
|-----------|---------|
| `CategorySidebar` | Category list with env/sound separator, active badges |
| `EnvironmentGrid` | Grid of environment buttons for current category |
| `EnvironmentButton` | Single button with icon, badges, volume slider for loops |
| `TopBar` | Search bar + time-of-day bar + status indicators |
| `SearchBar` | Fuzzy search with Ctrl+L focus |
| `TimeOfDayBar` | 4 time buttons (blank when unavailable) |
| `StopButtons` | Stop Lights / Stop Sound / Stop Atmosphere |
| `StatusBar` | Bottom bar showing active environment and sounds |
| `SettingsDialog` | 5-panel settings (Appearance, Spotify, WIZ Bulbs, Downloads, User Content) |
| `TimeVariantDialog` | Popup for selecting time variant on environment click |
| `VolumeSlider` | Volume control for loop sounds |
| `NowPlayingWidget` | Shows currently playing atmosphere sounds |
| `LightingPreviewWidget` | Visual preview of light colors |

**Types** (`ui/src/types/index.ts`):

TypeScript interfaces mirror Rust structs: `EnvironmentConfig`, `ActiveState`, `AvailableTimes`, `EnginesConfig`, etc. Helper functions: `isLoopSound()`, `hasSound()`, `hasSpotify()`, `hasAtmosphere()`, `hasLights()`, `cleanDisplayName()`.

## Category System

Categories are split into three groups defined in `state.rs`:

```rust
/// Environment categories - shown BEFORE the "-- SOUNDS --" separator.
const ENVIRONMENT_CATEGORIES: &[&str] = &[
    "tavern", "town", "interiors", "travel", "forest", "coastal",
    "dungeon", "combat", "spooky", "relaxation", "celestial",
];

/// Sound categories - shown AFTER the "-- SOUNDS --" separator.
const SOUND_CATEGORIES: &[&str] = &[
    "nature", "water", "fire", "wind", "storm", "crowd",
    "footsteps", "reactions", "combat_sfx", "ambient", "creatures",
    "misc", "freesound", "sounds",
];

/// Hidden categories - never shown in the UI.
const HIDDEN_CATEGORIES: &[&str] = &["hidden"];
```

**Filtering**: Environment categories exclude sound effects. Sound categories only show sound effects. The sidebar shows environment categories first, a separator, then sound categories. Hidden categories (containing "Startup" configs) are never displayed.

## Key Concepts

### Virtual Loop Configs

At startup, `generate_virtual_loop_configs()` in `state.rs` scans all atmosphere mixes across all environment configs. For each unique freesound URL, it creates a virtual `EnvironmentConfig` with `metadata.loop_sound: true` and `engines.sound.file` set to the URL. These appear in sound categories as toggleable loop buttons.

### Time-of-Day Variants

4 time periods: `morning`, `daytime` (default), `afternoon`, `evening`. Time variants are stored inline in the YAML config using `time_variants:` — see `env_conf/README.md` for the schema.

### Loop Sounds vs Regular Environments

Loop sounds are identified by `metadata.loop: true` OR `engines.sound.loop: true` in YAML. Clicking toggles the sound on/off (runs via atmosphere engine). Regular environment buttons trigger the full start_environment flow.

### Sound-Only vs Full Environments

- **Sound-only configs**: Have sound enabled but no lights/spotify/atmosphere. Play a one-shot sound without stopping anything else.
- **Full environments**: Stop existing atmosphere, start new sound/spotify/atmosphere/lights. Lights are hot-swapped without flashing to warm white.

### Startup Behavior

On app launch, `trigger_startup_environment()` searches all categories (including hidden) for a config named "Startup" (case-insensitive), falls back to "Travel", and starts it automatically. If no configs are found, the app starts with an empty state.

### Pre-Download for Atmosphere Sounds

When starting an environment, `pre_download_atmosphere()` checks if atmosphere URLs are cached. If not, it queues downloads and waits (up to 60s) with the inner lock released so `get_active_state` polling continues.

## User Content Directory

Users can add custom environments, sound collections, and audio files via a platform-standard user content directory. Content placed here is loaded alongside built-in configs.

### Platform Paths
- **Linux**: `~/.local/share/com.peterlesko.immerseyourself/`
- **macOS**: `~/Library/Application Support/com.peterlesko.immerseyourself/`
- **iOS**: Sandboxed app data directory

### Directory Structure (auto-created on first launch)
```
{user_content_dir}/
  env_conf/      # Environment YAML configs (same schema as built-in)
  sound_conf/    # Sound collection YAML configs
  sounds/        # Audio files (.wav, .mp3, .ogg, .opus, .flac)
  README.md      # Usage instructions
```

### Behavior
- **Override by filename**: User configs with the same filename as built-in configs override them
- **Additive**: User configs with unique names appear alongside built-in ones
- **Sound resolution**: SoundEngine searches user content dir as a fallback when resolving file paths
- **Settings UI**: Settings > User Content panel shows the directory path with an "Open Folder" button

## Environment Configuration (env_conf/)

See `env_conf/README.md` for the complete schema reference.

## Sound Variation System (sound_conf/)

See `sound_conf/README.md` for the schema and usage guide.

## Development Tasks

### Creating a New Environment

1. Create a YAML file in `env_conf/` or your user content directory following the schema
2. Restart the app to see the new environment

### Running the App

```bash
make dev     # Development with hot reload
make build   # Production build
make run     # Run pre-built binary
```

### Testing

```bash
make test    # Runs Rust tests via the Rust 1.89 wrapper
make check   # Type-check without building
```

### Frontend Development

The frontend is in `rust/immerse-tauri/ui/`. Standard React/Vite workflow, but always use `make dev` to start both frontend and backend together.

## Session Continuity

After compacted context or resuming a session, re-read the actual git log and file state before making changes. Do NOT rely solely on conversation summaries — they may be stale or inaccurate.

## Code Patterns

### Rust / Backend

- **State ownership**: All mutable state in `AppStateInner`. `AppState` wraps it with `tokio::Mutex`.
- **Sync-to-async bridge**: `AppState` methods use `runtime.block_on()` since Tauri commands are synchronous.
- **Lock discipline**: Release inner lock before long-running operations (downloads). Re-acquire after.
- **Engine spawning**: Use `runtime.spawn()` for fire-and-forget engine operations (lights, Spotify auth).
- **Async operations**: When implementing async operations (especially Spotify/network calls), always use `.await` instead of fire-and-forget `spawn`, and include required HTTP headers like `Content-Length`. Test network calls end-to-end before committing.
- **Config reading**: INI files read with `configparser` crate. `.spotify.ini`, `.wizbulb.ini`, `settings.ini`.

### React Frontend

- **State via hook**: `useAppState()` hook manages all Tauri IPC and state.
- **Polling**: `ActiveState` polled every 1s via `get_active_state` command.
- **Keyboard shortcuts**: Registered in `App.tsx` via `useEffect` with `keydown` listener.
- **TypeScript types**: Must match Rust structs in `state.rs`. Defined in `types/index.ts`.
- **Responsive**: Mobile breakpoint at 960px. Hamburger menu replaces sidebar.

### UI / CSS Conventions

For CSS tooltip implementations: avoid pseudo-element tooltips that can be clipped by `overflow` containers. Prefer JavaScript-positioned tooltips (e.g., absolutely positioned divs attached to `document.body`).

## Configuration Files

These files are NOT in git (see `.gitignore`):

### `.spotify.ini`
```ini
[DEFAULT]
username = <spotify_username>
client_id = <spotify_app_client_id>
client_secret = <spotify_app_client_secret>
redirectURI = http://127.0.0.1:8888/callback
```

### `.wizbulb.ini`
```ini
[DEFAULT]
backdrop_bulbs = 192.168.1.165 192.168.1.159 192.168.1.160
overhead_bulbs = 192.168.1.161 192.168.1.162
battlefield_bulbs = 192.168.1.163 192.168.1.164
```

### `settings.ini`
```ini
[spotify]
auto_start = ask  # Options: ask, start_local, use_remote, disabled

[downloads]
ignore_ssl_errors = false
```

## Debugging

- **Backend logs**: Set `RUST_LOG=debug` before running for detailed tracing output
- **Frontend console**: Browser DevTools console shows `[ENV_CLICK]`, `[BACKEND]`, `[TIME_SELECT]` prefixed logs
- **State polling**: Frontend polls `get_active_state` every second; check for stale state if UI seems unresponsive

### Common Issues

1. **Cargo build fails**: Never run `cargo` directly. Use `make test`, `make check`, `make build`.
2. **Lights not responding**: Check `.wizbulb.ini` exists and has correct IPs. Use Settings > WIZ Bulbs > Discover Bulbs.
3. **Spotify not working**: Check `.spotify.ini` credentials. OAuth tokens in `.cache` expire periodically.
4. **Missing atmosphere sounds**: Sounds are downloaded from freesound.org on first use and cached in `freesound.org/` directory.
5. **SSL errors on downloads**: Enable "Ignore SSL Errors" in Settings > Downloads panel.

## Known Issues

- Sound playback requires `ffplay` or `aplay` installed on the system
- WIZ bulbs may be unreachable on the network -- errors are logged but do not crash the app
- Spotify OAuth tokens expire periodically; delete `.cache` to re-authenticate
- Sound-only configs can run alongside lights configs; only lights configs replace each other
- Button shortcuts are per-category; same key triggers different buttons on different categories
- Virtual loop configs depend on cached filenames for display names; names may show as "Sound {id}" before first download

## iOS / Mobile

For iOS builds: files must be written to writable cache/documents directories, never the read-only app bundle. The `curl` command is not available on iOS — use `reqwest` or native HTTP clients. Always verify iOS-specific constraints (framework linking, sandbox paths) before marking a task complete.

## iOS Builds via GitHub Actions

Workflow file: `.github/workflows/ios-build.yml`

Required GitHub Secrets: `APPLE_CERTIFICATE_P12`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_PROVISIONING_PROFILE`, `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID`.

See the workflow file for full setup requirements.

## Claude Code Skills

Skills are invoked with `/skill-name` in Claude Code.

| Skill | Description |
|-------|-------------|
| `/desktop-release` | Cut a full release (desktop + iOS) — bumps version, tags, pushes, monitors CI. Options: `--minor`, `--major`, `--version X.Y.Z`. |
| `/ios-release` | Cut an iOS-only TestFlight release — bumps version, pushes to main (no tag). Delegates to `scripts/desktop-release.py --ios-only`. |
| `/build-check` | Run cross-platform build verification (3 parallel agents). |
| `/devlog` | Generate a diary-style devlog entry from git commits. |
