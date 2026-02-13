import type { FC } from 'react';
import type { AnimationConfig, LightGroupConfig } from '../types';
import { getSceneColor } from '../utils/sceneColors';

interface LightingPreviewProps {
  animationConfig: AnimationConfig | null;
  onClick?: () => void;
}

const BULB_GROUPS = ['backdrop', 'overhead', 'battlefield'] as const;

/**
 * Calculate the display color for a light group configuration.
 * Returns a CSS color string.
 */
function getGroupColor(
  config: LightGroupConfig | undefined,
  allGroups: Record<string, LightGroupConfig> | undefined
): string {
  if (!config) return 'transparent';

  switch (config.type) {
    case 'rgb': {
      const [r, g, b] = config.rgb.base;
      // Apply brightness as a scaling factor
      const avgBrightness = (config.brightness.min + config.brightness.max) / 2 / 255;
      return `rgb(${Math.round(r * avgBrightness)}, ${Math.round(g * avgBrightness)}, ${Math.round(b * avgBrightness)})`;
    }

    case 'scene': {
      // Use first scene ID for color
      const sceneId = config.scene_id ?? config.scenes?.ids?.[0];
      if (sceneId) {
        const [r, g, b] = getSceneColor(sceneId);
        const avgBrightness = config.brightness
          ? (config.brightness.min + config.brightness.max) / 2 / 255
          : 0.7;
        return `rgb(${Math.round(r * avgBrightness)}, ${Math.round(g * avgBrightness)}, ${Math.round(b * avgBrightness)})`;
      }
      return '#666';
    }

    case 'inherit_backdrop': {
      // Resolve to backdrop's color
      const backdropConfig = allGroups?.['backdrop'];
      if (backdropConfig && backdropConfig.type !== 'inherit_backdrop' && backdropConfig.type !== 'inherit_overhead') {
        return getGroupColor(backdropConfig, allGroups);
      }
      return '#666';
    }

    case 'inherit_overhead': {
      // Resolve to overhead's color
      const overheadConfig = allGroups?.['overhead'];
      if (overheadConfig && overheadConfig.type !== 'inherit_backdrop' && overheadConfig.type !== 'inherit_overhead') {
        return getGroupColor(overheadConfig, allGroups);
      }
      return '#666';
    }

    case 'off':
      return '#222';

    default:
      return '#666';
  }
}

/**
 * Determine if a color is light enough to need dark text.
 */
function isLightColor(cssColor: string): boolean {
  // Parse rgb(r, g, b) format
  const match = cssColor.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
  if (!match) return false;

  const r = parseInt(match[1], 10);
  const g = parseInt(match[2], 10);
  const b = parseInt(match[3], 10);

  // Calculate relative luminance
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return luminance > 0.5;
}

export const LightingPreviewWidget: FC<LightingPreviewProps> = ({ animationConfig, onClick }) => {
  const isActive = animationConfig !== null;
  const groups = animationConfig?.groups;

  // Get colors for gradient
  const backdropColor = isActive ? getGroupColor(groups?.['backdrop'], groups) : '#444';
  const overheadColor = isActive ? getGroupColor(groups?.['overhead'], groups) : '#444';
  const battlefieldColor = isActive ? getGroupColor(groups?.['battlefield'], groups) : '#444';

  // Calculate average luminance for text color
  const avgLuminance = isActive
    ? (isLightColor(backdropColor) ? 1 : 0) +
      (isLightColor(overheadColor) ? 1 : 0) +
      (isLightColor(battlefieldColor) ? 1 : 0)
    : 0;
  const textColor = avgLuminance >= 2 ? '#222' : '#fff';

  const gradientStyle = isActive
    ? {
        background: `linear-gradient(to bottom, ${backdropColor} 0%, ${overheadColor} 50%, ${battlefieldColor} 100%)`,
        color: textColor,
      }
    : {
        background: '#444',
        color: '#888',
      };

  return (
    <div
      className={`lighting-preview-widget ${isActive ? 'active clickable' : 'inactive'}`}
      onClick={isActive ? onClick : undefined}
      title={isActive ? 'Go to active lights (5)' : undefined}
      style={gradientStyle}
    >
      {isActive && (
        <div className="lighting-preview-badge">
          <span className="lighting-preview-badge-key">5</span>
          <span className="lighting-preview-badge-icon">💡</span>
        </div>
      )}
      {BULB_GROUPS.map((groupName) => (
        <div key={groupName} className="lighting-preview-row">
          <span className="lighting-preview-label">{groupName}</span>
        </div>
      ))}
    </div>
  );
};
