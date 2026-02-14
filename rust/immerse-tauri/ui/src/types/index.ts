// Types matching the Rust backend structures

export interface Metadata {
  tags: string[];
  intensity?: string;
  suitable_for: string[];
  loop?: boolean;  // "loop" in YAML - marks config as a toggleable loop sound
}

export interface SoundConfig {
  enabled: boolean;
  file: string;
  loop?: boolean;  // "loop" in YAML - sound plays as toggleable atmosphere loop
}

export interface SpotifyConfig {
  enabled: boolean;
  context_uri: string;
  offset?: {
    position?: number;
    uri?: string;
  };
}

export interface SoundMix {
  url: string;
  volume: number;
  name?: string;
  optional?: boolean;
  probability?: number;
  max_duration?: number;
  fade_duration?: number;
}

export interface AtmosphereConfig {
  enabled: boolean;
  min_sounds?: number;
  max_sounds?: number;
  mix: SoundMix[];
}

export interface RgbConfig {
  base: [number, number, number];
  variance: [number, number, number];
}

export interface BrightnessConfig {
  min: number;
  max: number;
}

export interface FlashConfig {
  probability: number;
  color?: [number, number, number];
  brightness?: number;
  duration?: number;
}

export interface RgbGroupConfig {
  rgb: RgbConfig;
  brightness: BrightnessConfig;
  flash?: FlashConfig;
}

export interface SceneGroupConfig {
  scenes?: {
    ids: number[];
    speed_min: number;
    speed_max: number;
  };
  scene_id?: number;
  speed?: number;
  brightness?: BrightnessConfig;
}

export type LightGroupConfig =
  | { type: 'rgb' } & RgbGroupConfig
  | { type: 'scene' } & SceneGroupConfig
  | { type: 'off' }
  | { type: 'inherit_backdrop' }
  | { type: 'inherit_overhead' };

export interface AnimationConfig {
  cycletime: number;
  groups: Record<string, LightGroupConfig>;
}

export interface LightsConfig {
  enabled: boolean;
  animation?: AnimationConfig;
}

export interface EnginesConfig {
  sound?: SoundConfig;
  spotify?: SpotifyConfig;
  atmosphere?: AtmosphereConfig;
  lights?: LightsConfig;
}

export interface EnvironmentConfig {
  name: string;
  category: string;
  description?: string;
  icon?: string;
  metadata?: Metadata;
  engines: EnginesConfig;
  time_variants?: Record<string, unknown>;
}

export interface ActiveState {
  active_lights_config: string | null;
  /** Name of the currently playing sound effect (entry sound or one-shot).
   * null if no sound is currently playing. */
  active_sound: string | null;
  active_atmosphere_urls: string[];
  /** Display names for active atmosphere sounds (cleaned up for UI). */
  atmosphere_names: string[];
  /** Names with author info for tooltips (e.g., "Sound Name by Author"). */
  atmosphere_names_with_author: string[];
  atmosphere_volumes: Record<string, number>;
  current_time: string;
  current_category: string;
  lights_available: boolean;
  spotify_available: boolean;
  is_downloading: boolean;
  pending_downloads: number;
  /** Available time variants for the active lights config.
   * Empty if no lights config is active or the config has no time variants. */
  available_times: string[];
  /** Whether sounds are currently paused (both sound engine and atmosphere). */
  is_sounds_paused: boolean;
  /** Incremented when categories/configs change. Watch this to refresh. */
  config_version: number;
}

export interface AvailableTimes {
  config_name: string;
  times: string[];
  has_variants: boolean;
}

// Helper functions
export function isLoopSound(config: EnvironmentConfig): boolean {
  // Check metadata.loop (matches Python: config.get("metadata", {}).get("loop", False))
  if (config.metadata?.loop) {
    return true;
  }
  // Check engines.sound.loop (matches Python: config.get("engines", {}).get("sound", {}).get("loop", False))
  if (config.engines.sound?.loop) {
    return true;
  }
  return false;
}

export function hasSound(config: EnvironmentConfig): boolean {
  return config.engines.sound?.enabled ?? false;
}

export function hasSpotify(config: EnvironmentConfig): boolean {
  return (config.engines.spotify?.enabled ?? false) &&
         (config.engines.spotify?.context_uri ?? '') !== '';
}

export function hasAtmosphere(config: EnvironmentConfig): boolean {
  return config.engines.atmosphere?.enabled ?? false;
}

export function hasLights(config: EnvironmentConfig): boolean {
  return (config.engines.lights?.enabled ?? false) &&
         config.engines.lights?.animation !== undefined;
}

export function getSoundUrl(config: EnvironmentConfig): string {
  return config.engines.sound?.file ?? '';
}

/**
 * Cleans up a display name for sounds/environments.
 * - Removes "freesound" prefix (case insensitive)
 * - Removes leading hyphens/spaces after prefix
 * - Removes "by ..." suffix
 * - Removes file extensions (.wav, .flac, .mp3, .ogg, .opus)
 * - Converts underscores to spaces
 */
export function cleanDisplayName(name: string): string {
  let result = name;

  // Convert underscores to spaces first
  result = result.replace(/_/g, ' ');

  // Remove "freesound" prefix (case insensitive) and any following hyphens/spaces
  result = result.replace(/^freesound\s*[-–—]?\s*/i, '');

  // Remove "by ..." suffix FIRST (before extension removal)
  // Find last occurrence of " by " and remove everything after
  const byIndex = result.toLowerCase().lastIndexOf(' by ');
  if (byIndex > 0) {
    result = result.substring(0, byIndex);
  }

  // Remove file extensions AFTER "by" removal
  result = result.replace(/\.(wav|flac|mp3|ogg|opus)$/i, '');

  return result.trim();
}

/**
 * Extracts author from a sound name (the part after "by ").
 * Returns null if no author found.
 */
export function extractAuthor(name: string): string | null {
  // Convert underscores to spaces
  const result = name.replace(/_/g, ' ');

  // Find "by " and extract author
  const byIndex = result.toLowerCase().lastIndexOf(' by ');
  if (byIndex > 0) {
    let author = result.substring(byIndex + 4).trim();
    // Remove file extension from author if present
    author = author.replace(/\.(wav|flac|mp3|ogg|opus)$/i, '');
    return author || null;
  }
  return null;
}
