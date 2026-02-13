import type { FC } from 'react';
import { SearchBar } from './SearchBar';
import { TimeOfDayBar } from './TimeOfDayBar';
import { LightingPreviewWidget } from './LightingPreviewWidget';
import { NowPlayingWidget } from './NowPlayingWidget';
import type { ActiveState, EnvironmentConfig, AnimationConfig } from '../types';

interface TopBarProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  onSearchSelect?: () => void;
  currentTime: string;
  availableTimes: string[];
  onTimeChange: (time: string) => void;
  activeState: ActiveState | null;
  allConfigs: Record<string, EnvironmentConfig[]>;
  onLightsClick?: () => void;
  showHamburger?: boolean;
  mobileMenuOpen?: boolean;
  onHamburgerClick?: () => void;
}

/**
 * Deep merge two objects. Used to apply time variant overrides.
 */
function deepMerge<T>(base: T, override: Partial<T> | undefined): T {
  if (!override) return base;
  if (typeof base !== 'object' || base === null) return (override as T) ?? base;
  if (typeof override !== 'object' || override === null) return override as T;

  const result = { ...base } as Record<string, unknown>;
  for (const key of Object.keys(override)) {
    const baseVal = (base as Record<string, unknown>)[key];
    const overrideVal = (override as Record<string, unknown>)[key];
    if (typeof baseVal === 'object' && baseVal !== null && typeof overrideVal === 'object' && overrideVal !== null && !Array.isArray(overrideVal)) {
      result[key] = deepMerge(baseVal, overrideVal);
    } else if (overrideVal !== undefined) {
      result[key] = overrideVal;
    }
  }
  return result as T;
}

export const TopBar: FC<TopBarProps> = ({
  searchQuery,
  onSearchChange,
  onSearchSelect,
  currentTime,
  availableTimes,
  onTimeChange,
  activeState,
  allConfigs,
  onLightsClick,
  showHamburger = false,
  mobileMenuOpen = false,
  onHamburgerClick,
}) => {
  // Find the icon and animation config for the active lights config
  let activeLightsIcon: string | undefined;
  let activeLightsAnimation: AnimationConfig | null = null;
  if (activeState?.active_lights_config) {
    for (const configs of Object.values(allConfigs)) {
      const config = configs.find(c => c.name === activeState.active_lights_config);
      if (config) {
        activeLightsIcon = config.icon;

        // Get base animation config
        let animation = config.engines.lights?.animation ?? null;

        // Apply time variant overrides if not daytime
        if (animation && currentTime !== 'daytime' && config.time_variants) {
          const timeVariant = config.time_variants[currentTime] as { engines?: { lights?: { animation?: Partial<AnimationConfig> } } } | undefined;
          if (timeVariant?.engines?.lights?.animation) {
            animation = deepMerge(animation, timeVariant.engines.lights.animation);
          }
        }

        activeLightsAnimation = animation;
        break;
      }
    }
  }

  return (
    <div className="top-bar">
      <div className="top-bar-left">
        {showHamburger && (
          <button
            className={`hamburger-button${mobileMenuOpen ? ' open' : ''}`}
            onClick={onHamburgerClick}
            title={mobileMenuOpen ? 'Show environments' : 'Show categories'}
            aria-label={mobileMenuOpen ? 'Show environments' : 'Show categories'}
          >
            ☰
          </button>
        )}
        <SearchBar value={searchQuery} onChange={onSearchChange} onSelect={onSearchSelect} />
      </div>

      <div className="top-bar-center">
        <TimeOfDayBar
          currentTime={currentTime}
          availableTimes={availableTimes}
          onTimeChange={onTimeChange}
        />
      </div>

      <div className="top-bar-lights">
        <LightingPreviewWidget animationConfig={activeLightsAnimation} onClick={onLightsClick} />
      </div>

      <div className="top-bar-right">
        <NowPlayingWidget
          activeLightsConfig={activeState?.active_lights_config ?? null}
          activeLightsIcon={activeLightsIcon}
          activeAtmosphereCount={activeState?.active_atmosphere_urls.length ?? 0}
          isDownloading={activeState?.is_downloading ?? false}
          pendingDownloads={activeState?.pending_downloads ?? 0}
          onLightsClick={onLightsClick}
        />
      </div>
    </div>
  );
};
