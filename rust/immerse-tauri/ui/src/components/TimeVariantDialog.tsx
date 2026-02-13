import { useEffect, type FC } from 'react';

interface TimeVariantDialogProps {
  configName: string;
  availableTimes: string[];
  currentTime: string;
  onSelect: (time: string) => void;
  onClose: () => void;
}

// TIME_META matches Python's TIME_PERIODS: morning, daytime, afternoon, evening
const TIME_META: Record<string, { icon: string; label: string; shortcut: string }> = {
  morning: { icon: '🌅', label: 'Morning', shortcut: '1' },
  daytime: { icon: '☀️', label: 'Daytime', shortcut: '2' },
  afternoon: { icon: '🌤️', label: 'Afternoon', shortcut: '3' },
  evening: { icon: '🌙', label: 'Evening', shortcut: '4' },
};

export const TimeVariantDialog: FC<TimeVariantDialogProps> = ({
  configName,
  availableTimes,
  currentTime,
  onSelect,
  onClose,
}) => {
  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
        return;
      }

      // Number shortcuts for times (matches Python TIME_PERIODS)
      const shortcutMap: Record<string, string> = {
        '1': 'morning',
        '2': 'daytime',
        '3': 'afternoon',
        '4': 'evening',
      };

      if (e.key in shortcutMap) {
        const time = shortcutMap[e.key];
        if (availableTimes.includes(time)) {
          onSelect(time);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [availableTimes, onSelect, onClose]);

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog-content time-variant-dialog" onClick={e => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>Select Time of Day</h2>
          <p className="dialog-subtitle">{configName}</p>
        </div>

        <div className="time-variant-options">
          {availableTimes.map(time => {
            const meta = TIME_META[time] || { icon: '⏰', label: time, shortcut: '' };
            const isSelected = time === currentTime;

            return (
              <button
                key={time}
                className={`time-variant-option ${isSelected ? 'selected' : ''}`}
                onClick={() => onSelect(time)}
              >
                <span className="time-variant-shortcut">{meta.shortcut}</span>
                <span className="time-variant-icon">{meta.icon}</span>
                <span className="time-variant-label">{meta.label}</span>
              </button>
            );
          })}
        </div>

        <div className="dialog-footer">
          <button className="dialog-cancel" onClick={onClose}>
            Cancel (Esc)
          </button>
        </div>
      </div>
    </div>
  );
};
