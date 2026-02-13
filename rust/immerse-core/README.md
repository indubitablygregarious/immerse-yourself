# immerse-core

Shared core library for the Immerse Yourself ambient environment system. Provides YAML config loading/validation, audio engines, smart light control, Spotify integration, and a background download queue for freesound.org sounds.

## Module Architecture

```mermaid
graph TD
    subgraph "immerse-core"
        LIB[lib.rs / prelude]
        CFG[config/]
        ENG[engines/]
        DLQ[download_queue]
        ERR[error]
        FFI[ffi]
    end

    LIB --> CFG
    LIB --> ENG
    LIB --> DLQ
    LIB --> ERR
    LIB --> FFI

    ENG --> CFG
    ENG --> DLQ
    ENG --> ERR

    CFG --> ERR
    DLQ --> ERR
```

## Modules

### `config/` -- Configuration Loading and Validation

Loads 340+ environment YAML files, validates them, and provides typed Rust structs. Handles time-of-day variant resolution and caching.

Key types: `EnvironmentConfig`, `EnginesConfig`, `AnimationConfig`, `LightGroupConfig`, `TimeOfDay`

See [`src/config/README.md`](./src/config/README.md) for details.

### `engines/` -- Audio, Lights, and Spotify Engines

Four engine implementations for controlling the environment:

- **SoundEngine** -- One-shot and async audio playback via ffplay/paplay/aplay
- **AtmosphereEngine** -- Looping ambient sound mixes with volume control and fade-out
- **LightsEngine** -- WIZ smart bulb animation via UDP
- **SpotifyEngine** -- Spotify Web API playback control with OAuth

See [`src/engines/README.md`](./src/engines/README.md) for details.

### `download_queue` -- Background Download Queue

Manages async downloads of freesound.org sounds. Downloads are processed one at a time in a background thread. Supports caching, status tracking, and callbacks on completion.

Key types: `DownloadQueue`, `DownloadStatus`, `DownloadCallback`

### `error` -- Error Types

Centralized error enum using `thiserror`. Covers config, sound, Spotify, lights, atmosphere, daemon, and I/O errors.

### `ffi` -- Foreign Function Interface

FFI layer for Swift/iOS interop, exposing core functionality to non-Rust consumers.

## Engine Lifecycle

```mermaid
sequenceDiagram
    participant App as Tauri App
    participant SE as SoundEngine
    participant AE as AtmosphereEngine
    participant DQ as DownloadQueue
    participant LE as LightsEngine
    participant SP as SpotifyEngine

    App->>SE: play_async("sound_conf:transition")
    SE->>SE: resolve_sound_conf() -> random pick
    SE->>SE: spawn ffplay subprocess

    App->>AE: start_single(url, volume)
    AE->>DQ: enqueue_or_get_cached(url)
    alt Cached
        DQ-->>AE: Some(path)
        AE->>AE: spawn ffplay --loop
    else Not cached
        DQ-->>AE: None
        DQ->>DQ: background download (yt-dlp/curl)
        DQ-->>AE: callback(Ok(path))
        AE->>AE: spawn ffplay --loop
    end

    App->>LE: start(AnimationConfig)
    LE->>LE: tokio::spawn animation_loop
    loop Every cycletime
        LE->>LE: generate_pilot per group
        LE->>LE: UDP send to bulbs (fire-and-forget)
    end

    App->>SP: authenticate()
    SP->>SP: load cached token or OAuth flow
    App->>SP: play_context(uri)
    SP->>SP: PUT /me/player/play
```

## Key Types

| Type | Module | Purpose |
|------|--------|---------|
| `EnvironmentConfig` | config | Complete environment definition from YAML |
| `ConfigLoader` | config | Discovers, loads, validates, and caches configs |
| `ConfigValidator` | config | Validates config constraints (brightness ranges, URIs, etc.) |
| `TimeOfDay` | config | Enum: Morning, Daytime, Afternoon, Evening |
| `SoundEngine` | engines | Plays audio files via system subprocess |
| `AtmosphereEngine` | engines | Manages looping ambient sounds with fade/duration |
| `LightsEngine` | engines | Controls WIZ bulbs with async animation loop |
| `SpotifyEngine` | engines | Spotify Web API client with OAuth token management |
| `DownloadQueue` | download_queue | Background download manager for freesound.org |

## Building

Always use Make targets from the project root (never `cargo` directly):

```bash
make tauri-test     # Run tests
make tauri-check    # Type-check
```
