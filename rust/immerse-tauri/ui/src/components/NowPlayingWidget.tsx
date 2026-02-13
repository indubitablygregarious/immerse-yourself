import type { FC } from 'react';

interface NowPlayingWidgetProps {
  activeLightsConfig: string | null;
  activeLightsIcon?: string;
  activeAtmosphereCount: number;
  isDownloading: boolean;
  pendingDownloads: number;
  onLightsClick?: () => void;
}

export const NowPlayingWidget: FC<NowPlayingWidgetProps> = ({
  activeLightsConfig,
  activeLightsIcon,
  activeAtmosphereCount,
  isDownloading,
  pendingDownloads,
  onLightsClick,
}) => {
  // Determine current state and display
  let statusText = 'idle';
  let icon = '⏸';
  let isClickable = false;

  if (isDownloading) {
    statusText = `downloading (${pendingDownloads})`;
    icon = '⬇️';
  } else if (activeLightsConfig) {
    statusText = activeLightsConfig;
    icon = activeLightsIcon || '💡';
    isClickable = true;
  } else if (activeAtmosphereCount > 0) {
    statusText = `${activeAtmosphereCount} sound${activeAtmosphereCount > 1 ? 's' : ''}`;
    icon = '🔊';
  }

  const handleClick = () => {
    if (isClickable && onLightsClick) {
      onLightsClick();
    }
  };

  return (
    <div className="now-playing-widget">
      <span className="now-playing-title">now playing:</span>
      <div
        className={`now-playing-content ${isClickable ? 'clickable' : ''} ${isDownloading ? 'downloading' : ''}`}
        onClick={handleClick}
      >
        <span className="now-playing-status">{statusText}</span>
        <span className="now-playing-icon">{icon}</span>
      </div>
      {isClickable && (
        <span className="lighting-preview-badge">
          <span className="lighting-preview-badge-key">5</span>
          <span className="lighting-preview-badge-icon">💡</span>
        </span>
      )}
    </div>
  );
};
