/**
 * WIZ scene ID to RGB color mapping.
 * Maps scene IDs to representative colors for UI preview.
 */

const SCENE_COLORS: Record<number, [number, number, number]> = {
  1: [0, 100, 180],       // Ocean - blue
  2: [200, 80, 100],      // Romance - pink/red
  3: [255, 140, 50],      // Sunset - orange
  4: [255, 100, 200],     // Party - vibrant pink
  5: [255, 100, 30],      // Fireplace - warm orange
  6: [255, 180, 100],     // Cozy - warm amber
  7: [50, 150, 50],       // Forest - green
  8: [200, 180, 220],     // Pastel colors - soft purple
  9: [255, 220, 150],     // Wake-up - soft warm
  10: [100, 80, 150],     // Bedtime - dim purple
  11: [255, 220, 180],    // Warm white
  12: [255, 255, 220],    // Daylight - bright white/yellow
  13: [220, 240, 255],    // Cool white - blueish white
  14: [80, 60, 100],      // Night light - dim purple
  15: [255, 255, 255],    // Focus - white
  16: [180, 150, 200],    // Relax - soft purple
  17: [255, 200, 150],    // True colors - warm
  18: [100, 120, 180],    // TV time - blue tint
  19: [100, 200, 100],    // Plantgrowth - green
  20: [150, 220, 150],    // Spring - light green
  21: [255, 220, 100],    // Summer - warm yellow
  22: [200, 120, 50],     // Fall - orange/brown
  23: [30, 80, 150],      // Deep dive - deep blue
  24: [80, 180, 80],      // Jungle - green
  25: [150, 255, 150],    // Mojito - mint green
  26: [255, 180, 80],     // Candlelight - amber
  27: [255, 50, 50],      // Christmas - red
  28: [255, 100, 0],      // Halloween - orange
  29: [255, 180, 80],     // Candlelight (duplicate)
  30: [255, 220, 100],    // Golden white - warm yellow
  31: [200, 50, 100],     // Pulse - purple/pink
  32: [180, 140, 100],    // Steampunk - bronze
  33: [255, 180, 50],     // Diwali - warm amber
  34: [255, 255, 255],    // White
  35: [255, 0, 0],        // Alarm - red
  1000: [150, 100, 200],  // Rhythm - purple
};

// Default color for unknown scene IDs
const DEFAULT_SCENE_COLOR: [number, number, number] = [128, 128, 128];

/**
 * Get the representative RGB color for a WIZ scene ID.
 * @param sceneId The WIZ scene ID
 * @returns RGB tuple [r, g, b] with values 0-255
 */
export function getSceneColor(sceneId: number): [number, number, number] {
  return SCENE_COLORS[sceneId] ?? DEFAULT_SCENE_COLOR;
}
