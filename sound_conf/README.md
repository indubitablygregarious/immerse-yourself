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
    fadeout: 4000                      # Optional: fade out duration in milliseconds
```

Each entry must have either `file` (local path) or `url` (freesound.org link). The optional `volume` and `fadeout` fields are returned alongside the sound path when present.

## Adding a New Collection

1. Create a new YAML file in this directory (e.g., `thunder.yaml`).
2. Follow the schema above, mixing local files and freesound.org URLs.
3. Reference it from any environment config with `file: "sound_conf:thunder"`.

Sound collections can also be placed in your user content directory:
- **Linux**: `~/.local/share/com.peterlesko.immerseyourself/sound_conf/`
- **macOS**: `~/Library/Application Support/com.peterlesko.immerseyourself/sound_conf/`
