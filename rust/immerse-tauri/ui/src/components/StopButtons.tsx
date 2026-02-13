import type { FC } from 'react';
import { useEffect } from 'react';

interface StopButtonsProps {
  onStopLights: () => void;
  onTogglePause: () => void;
  lightsActive: boolean;
  soundsActive: boolean;
  isPaused: boolean;
  isMobileMode: boolean;
}

export const StopButtons: FC<StopButtonsProps> = ({
  onStopLights,
  onTogglePause,
  lightsActive,
  soundsActive,
  isPaused,
  isMobileMode,
}) => {
  // Handle spacebar to toggle pause (desktop only)
  useEffect(() => {
    if (isMobileMode) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === 'Space' && !isInputFocused()) {
        e.preventDefault();
        onTogglePause();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onTogglePause, isMobileMode]);

  return (
    <div className="stop-buttons">
      <button
        className={`stop-button stop-lights ${lightsActive ? 'active' : ''}`}
        onClick={onStopLights}
        title="Stop light animations"
      >
        <span className="stop-icon">💡</span>
        <span className="stop-label">STOP LIGHTS</span>
      </button>

      <button
        className={`stop-button stop-pause ${isPaused ? 'paused' : ''} ${soundsActive ? 'active' : ''}`}
        onClick={onTogglePause}
        title={isPaused ? 'Resume all sounds (Spacebar)' : 'Pause all sounds (Spacebar)'}
      >
        <span className="stop-icon">{isPaused ? '\u25B6' : '\u23F8'}</span>
        <span className="stop-label">{isPaused ? 'PLAY' : 'PAUSE'}</span>
        <span className="shortcut-badge">Space</span>
      </button>
    </div>
  );
};

// Helper to check if an input is focused
function isInputFocused(): boolean {
  const active = document.activeElement;
  return active instanceof HTMLInputElement ||
         active instanceof HTMLTextAreaElement ||
         active instanceof HTMLSelectElement;
}
