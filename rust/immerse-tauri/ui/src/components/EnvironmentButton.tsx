import { FC, MouseEvent, useMemo, useState, useRef, useCallback } from 'react';
import { createPortal } from 'react-dom';
import type { EnvironmentConfig } from '../types';
import { isLoopSound, hasSound, hasSpotify, hasAtmosphere, hasLights, cleanDisplayName } from '../types';
import { VolumeSlider } from './VolumeSlider';

// Pastel badge colors matching Python's AppColors
const BADGE_PASTEL_COLORS = [
  '#FFD4D4', // Pink
  '#D4FFD4', // Green
  '#D4D4FF', // Blue
  '#FFE4D4', // Peach
  '#FFF9B0', // Yellow
  '#B4F0A8', // Mint
  '#B4E8F0', // Cyan
  '#E0B4F0', // Lavender
];

// Get a deterministic but varied color based on config name
const getBadgeColor = (name: string): string => {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = ((hash << 5) - hash) + name.charCodeAt(i);
    hash = hash & hash;
  }
  return BADGE_PASTEL_COLORS[Math.abs(hash) % BADGE_PASTEL_COLORS.length];
};

interface EnvironmentButtonProps {
  config: EnvironmentConfig;
  isActive: boolean;
  isLoopActive: boolean;
  isFocused?: boolean;
  shortcutKey?: string;
  volume: number;
  onVolumeChange: (volume: number) => void;
  onClick: () => void;
}

export const EnvironmentButton: FC<EnvironmentButtonProps> = ({
  config,
  isActive,
  isLoopActive,
  isFocused,
  shortcutKey,
  volume,
  onVolumeChange,
  onClick,
}) => {
  const isLoop = isLoopSound(config);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);
  const hideTimeout = useRef<ReturnType<typeof setTimeout>>();

  // Get a deterministic pastel color for this button's badge
  const badgeColor = useMemo(() => getBadgeColor(config.name), [config.name]);

  const handleClick = (e: MouseEvent) => {
    // Don't trigger if clicking on volume slider
    if ((e.target as HTMLElement).closest('.volume-slider')) {
      return;
    }
    onClick();
  };

  const updateTooltip = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    clearTimeout(hideTimeout.current);
    setTooltipPos({ x: e.clientX, y: e.clientY - 12 });
  }, []);

  const hideTooltip = useCallback(() => {
    hideTimeout.current = setTimeout(() => setTooltipPos(null), 100);
  }, []);

  return (
    <div className="env-button-container">
      {/* Main clickable button area */}
      <div
        className={`env-button ${isActive ? 'active' : ''} ${isLoopActive ? 'loop-active' : ''} ${isFocused ? 'focused' : ''}`}
        onClick={handleClick}
        role="button"
        tabIndex={0}
        onMouseEnter={updateTooltip}
        onMouseMove={updateTooltip}
        onMouseLeave={hideTooltip}
        onKeyDown={(e) => {
          // Don't trigger if pressing keys on volume slider
          if ((e.target as HTMLElement).closest('.volume-slider')) {
            return;
          }
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onClick();
          }
        }}
      >
        {/* Background icon - renders perfectly in webview */}
        {config.icon && (
          <span className="env-icon">{config.icon}</span>
        )}

        {/* Shortcut badge with pastel background */}
        {shortcutKey && (
          <span className="shortcut-badge" style={{ backgroundColor: badgeColor }}>{shortcutKey}</span>
        )}

        {/* Name - cleaned up for display */}
        <span className="env-name">{cleanDisplayName(config.name)}</span>

        {/* Feature badges */}
        <div className="badges">
          {hasSound(config) && (
            <span className="badge sound" title={isLoop ? "Loop sound" : "Sound effect"}>
              {/* Use megaphone for sound-only configs, speaker otherwise */}
              {!hasSpotify(config) && !hasAtmosphere(config) && !hasLights(config) ? '📢' : '🔊'}
            </span>
          )}
          {hasSpotify(config) && <span className="badge spotify" title="Spotify music">🎵</span>}
          {hasAtmosphere(config) && <span className="badge atmosphere" title="Atmosphere sounds">🌊</span>}
          {hasLights(config) && <span className="badge lights" title="Light animations">💡</span>}
          {isLoop && <span className="badge loop" title="Loop sound (toggleable)">🔁</span>}
        </div>

        {/* Volume slider for loop sounds - always visible on loop buttons (matches Python) */}
        {isLoop && (
          <VolumeSlider
            value={volume}
            onChange={onVolumeChange}
            onClick={(e) => e.stopPropagation()}
          />
        )}
      </div>

      {/* Description box below button - separate element like Python */}
      {config.description && (
        <div className="env-description-box">{config.description}</div>
      )}

      {/* Centered tooltip via portal */}
      {tooltipPos && createPortal(
        <div
          className="env-tooltip"
          style={{ left: tooltipPos.x, top: tooltipPos.y }}
        >
          <div className="tooltip-name">{config.name}</div>
          {config.description && <div className="tooltip-desc">{config.description}</div>}
        </div>,
        document.body
      )}
    </div>
  );
};
