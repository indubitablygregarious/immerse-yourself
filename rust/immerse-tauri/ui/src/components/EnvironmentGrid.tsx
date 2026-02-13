import type { FC } from 'react';
import type { EnvironmentConfig, ActiveState } from '../types';
import { isLoopSound, getSoundUrl } from '../types';
import { EnvironmentButton } from './EnvironmentButton';

interface EnvironmentGridProps {
  environments: EnvironmentConfig[];
  activeState: ActiveState | null;
  focusedIndex?: number | null;
  highlightedConfig?: string | null;
  onStartEnvironment: (config: EnvironmentConfig) => void;
  onToggleLoop: (url: string) => void;
  onVolumeChange: (url: string, volume: number) => void;
}

// Shortcut keys for buttons (Q, W, E, R, T, Y, U, I, O, P, A, S, D, F, G, H, J, K, L)
const SHORTCUT_KEYS = 'QWERTYUIOPASDFGHJKL'.split('');

export const EnvironmentGrid: FC<EnvironmentGridProps> = ({
  environments,
  activeState,
  focusedIndex,
  highlightedConfig,
  onStartEnvironment,
  onToggleLoop,
  onVolumeChange,
}) => {
  if (environments.length === 0) {
    return (
      <div className="env-grid-empty">
        <p>No environments found.</p>
        <p>Add environment configs to <code>env_conf/</code> or your <a href="#" onClick={(e) => { e.preventDefault(); }}>user content directory</a>.</p>
      </div>
    );
  }

  return (
    <div className="env-grid">
      {environments.map((config, index) => {
        const isLoop = isLoopSound(config);
        const soundUrl = getSoundUrl(config);

        const isLightsActive = activeState?.active_lights_config === config.name;
        const isLoopActive = isLoop && (activeState?.active_atmosphere_urls.includes(soundUrl) ?? false);
        const volume = activeState?.atmosphere_volumes[soundUrl] ?? 70;
        const shortcutKey = index < SHORTCUT_KEYS.length ? SHORTCUT_KEYS[index] : undefined;

        const handleClick = () => {
          if (isLoop) {
            onToggleLoop(soundUrl);
          } else {
            onStartEnvironment(config);
          }
        };

        const handleVolumeChange = (newVolume: number) => {
          onVolumeChange(soundUrl, newVolume);
        };

        // Button is focused if it matches focusedIndex (search) or highlightedConfig (badge click)
        const isFocused = focusedIndex === index || highlightedConfig === config.name;

        return (
          <EnvironmentButton
            key={config.name}
            config={config}
            isActive={isLightsActive}
            isLoopActive={isLoopActive}
            isFocused={isFocused}
            shortcutKey={shortcutKey}
            volume={volume}
            onVolumeChange={handleVolumeChange}
            onClick={handleClick}
          />
        );
      })}
    </div>
  );
};
