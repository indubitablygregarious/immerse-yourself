# Environment Configuration Schema

This directory contains YAML configuration files for Immerse Yourself environments. Each YAML file defines how an environment behaves: what sound effects to play, which Spotify playlist to use, what atmosphere loops to mix, and how to animate the lights.

## Quick Start

Create a new environment by creating a YAML file in this directory (or in your [user content directory](#user-content-directory)).

**Minimal Example:**
```yaml
name: "My Environment"
category: "tavern"
icon: "🎮"

metadata:
  tags: ["custom"]
  intensity: "medium"

engines:
  sound:
    enabled: true
    file: "sounds/door.wav"

  spotify:
    enabled: true
    context_uri: "spotify:playlist:YOUR_PLAYLIST_ID"

  lights:
    enabled: true
    animation:
      cycletime: 12
      groups:
        backdrop:
          type: "rgb"
          rgb:
            base: [128, 128, 128]
            variance: [20, 20, 20]
          brightness:
            min: 100
            max: 255
```

## Complete Schema Reference

### Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Display name shown in launcher |
| `category` | string | Yes | Category for sidebar organization (see categories below) |
| `description` | string | No | Short description |
| `icon` | string | No | Emoji displayed as semi-transparent background on button |
| `metadata` | object | No | Additional metadata for UI/filtering |
| `engines` | object | Yes | Configuration for sound, spotify, atmosphere, and lights engines |
| `time_variants` | object | No | Time-of-day overrides (morning, afternoon, evening) |

### Categories

**Environment categories** (shown before the sounds separator):
`tavern`, `town`, `interiors`, `travel`, `forest`, `coastal`, `dungeon`, `combat`, `spooky`, `relaxation`, `celestial`

**Sound categories** (shown after the sounds separator):
`nature`, `water`, `fire`, `wind`, `storm`, `crowd`, `footsteps`, `reactions`, `combat_sfx`, `ambient`, `creatures`, `misc`, `freesound`, `sounds`

### Metadata Fields

| Field | Type | Description |
|-------|------|-------------|
| `tags` | array of strings | Tags for filtering and search |
| `intensity` | string | Intensity level: `low`, `medium`, `high` |
| `suitable_for` | array of strings | Use cases |
| `loop` | boolean | If true, button toggles a loop sound on/off |

### Engines Configuration

#### Sound Engine

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `enabled` | boolean | Yes | Whether to play sound effect |
| `file` | string | If enabled | Path to sound file, `sound_conf:name` reference, or freesound.org URL |
| `volume` | integer | No | Volume level 1-100 (default: 80) |
| `loop` | boolean | No | If true, loops as atmosphere-style sound |

#### Spotify Engine

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `enabled` | boolean | Yes | Whether to play Spotify content |
| `context_uri` | string | If enabled | Spotify URI (playlist/album/episode) |

#### Atmosphere Engine

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `enabled` | boolean | Yes | Whether to play atmosphere loops |
| `mix` | array | If enabled | List of atmosphere sound entries |

Each mix entry:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | Yes | freesound.org URL |
| `volume` | integer | No | Volume 1-100 (default: 70) |
| `name` | string | No | Display name for the sound |
| `max_duration` | integer | No | Hard stop after N seconds |
| `fade_duration` | integer | No | Fade out over N seconds |

#### Lights Engine

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `enabled` | boolean | Yes | Whether to control lights |
| `animation` | object | If enabled | Animation configuration |

**Animation Configuration:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cycletime` | number | 12 | Seconds between light updates |
| `groups` | object | - | Configuration for each bulb group |

**Bulb Groups:** `backdrop`, `overhead`, `battlefield`

Each group:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | `rgb`, `scene`, `off`, `inherit_backdrop`, `inherit_overhead` |
| `rgb` | object | If type=rgb | `base: [R,G,B]`, `variance: [R,G,B]` |
| `brightness` | object | No | `min` and `max` (0-255) |
| `flash` | object | No | `probability`, `color: [R,G,B]`, `brightness`, `duration` |

For `type: scene`:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `ids` | array | [5,28,31] | WIZ scene IDs |
| `speed_min` | number | 10 | Min animation speed (1-200) |
| `speed_max` | number | 190 | Max animation speed (1-200) |

### Time-of-Day Variants

Override specific fields for different times of day (deep-merged into base config):

```yaml
time_variants:
  morning:
    engines:
      lights:
        animation:
          groups:
            backdrop:
              rgb: { base: [255, 200, 100] }
  evening:
    engines:
      lights:
        animation:
          groups:
            backdrop:
              rgb: { base: [20, 20, 60] }
```

`daytime` uses the base config as-is. Available times: `morning`, `daytime`, `afternoon`, `evening`.

## User Content Directory

You can also place configs in your user content directory:
- **Linux**: `~/.local/share/com.peterlesko.immerseyourself/env_conf/`
- **macOS**: `~/Library/Application Support/com.peterlesko.immerseyourself/env_conf/`

Configs with the same filename as built-in configs override them. Open via Settings > User Content.

## Tuning Guide

### Animation Speed
- **Very Slow (60s+)**: Meditation, background
- **Slow (20-40s)**: Social scenes, taverns
- **Medium (6-12s)**: Travel, exploration
- **Fast (2-4s)**: Combat
- **Very Fast (<2s)**: Panic, chaos

### Flash Probability
- **0.01-0.05**: Rare, subtle flickers
- **0.05-0.15**: Occasional interest
- **0.15-0.30**: Frequent intensity
- **0.30+**: Constant (extreme effects only)
