# Sound Configuration (sound_conf)

Sound variation system that provides randomized entry sounds for environments. Instead of playing the same sound every time a scene loads, `sound_conf` collections let you define a pool of sounds from which one is randomly selected at runtime.

## How It Works

Environment YAML files reference a sound collection using the `sound_conf:` prefix:

```yaml
engines:
  sound:
    enabled: true
    file: "sound_conf:squeaky_door"
```

At runtime, the resolver loads the corresponding YAML file (`sound_conf/squeaky_door.yaml`), picks a random entry from its `sounds` list, and returns either a local file path or a freesound.org URL for playback.

## YAML Schema

```yaml
name: "Collection Name"
description: "What these sounds are for"

sounds:
  - file: "sounds/dooropen.wav"       # Local file reference
    description: "Original door sound"

  - url: "https://freesound.org/..."  # Freesound URL (auto-downloaded and cached)
    description: "Door creak variant"
    volume: 60                         # Optional: playback volume (1-100)
    max_duration: 2000                 # Optional: max playback duration in milliseconds
    fadeout: 4000                      # Optional: fade out duration in milliseconds
    start_offset: 1500                 # Optional: start position in ms (for audio sprite sheets)
```

Each entry must have either `file` (local path) or `url` (freesound.org link). The optional `volume`, `max_duration`, `fadeout`, and `start_offset` fields control playback behavior.

### Audio Sprite Sheets

A single sound file can be sliced into multiple entries using `start_offset` and `max_duration`. This is useful when one file contains several distinct sounds packed together (e.g., a series of vocal whooshes). Each entry points to the same file/URL but plays a different segment:

```yaml
sounds:
  - url: "https://freesound.org/..."
    description: "Whoosh segment 1"
    start_offset: 0
    max_duration: 233

  - url: "https://freesound.org/..."
    description: "Whoosh segment 2"
    start_offset: 626
    max_duration: 147
```

## Adding a New Collection

1. Create a new YAML file in this directory (e.g., `thunder.yaml`).
2. Follow the schema above, mixing local files and freesound.org URLs.
3. Reference it from any environment config with `file: "sound_conf:thunder"`.

Sound collections can also be placed in your user content directory:
- **Linux**: `~/.local/share/com.peterlesko.immerseyourself/sound_conf/`
- **macOS**: `~/Library/Application Support/com.peterlesko.immerseyourself/sound_conf/`
