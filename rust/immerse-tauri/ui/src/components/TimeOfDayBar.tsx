import type { FC } from 'react';

interface TimeOfDayBarProps {
  currentTime: string;
  availableTimes: string[];
  onTimeChange: (time: string) => void;
}

// TIME_DATA matches Python's TIME_PERIODS: morning, daytime, afternoon, evening
// "daytime" is the default - uses base config without time variant overrides
const TIME_DATA = [
  { key: 'morning', icon: '🌅', label: 'Morning', shortcut: '1' },
  { key: 'daytime', icon: '☀️', label: 'Daytime', shortcut: '2' },
  { key: 'afternoon', icon: '🌤️', label: 'Afternoon', shortcut: '3' },
  { key: 'evening', icon: '🌙', label: 'Evening', shortcut: '4' },
];

export const TimeOfDayBar: FC<TimeOfDayBarProps> = ({
  currentTime,
  availableTimes,
  onTimeChange,
}) => {
  // When availableTimes is empty, NO times are available (all buttons blank)
  // This matches Python's behavior where empty list means config has no time variants
  const hasAnyVariants = availableTimes.length > 0;

  return (
    <div className="time-of-day-bar">
      {TIME_DATA.map(({ key, icon, label, shortcut }) => {
        // Button is only available if there are variants AND this time is in the list
        const isAvailable = hasAnyVariants && availableTimes.includes(key);
        const isSelected = currentTime === key;

        return (
          <button
            key={key}
            className={`time-button ${isSelected ? 'selected' : ''} ${!isAvailable ? 'unavailable' : ''}`}
            onClick={() => isAvailable && onTimeChange(key)}
            disabled={!isAvailable}
            title={isAvailable ? `${label} (${shortcut})` : ''}
          >
            {isAvailable && <span className="time-shortcut">{shortcut}</span>}
            <span className="time-icon">{isAvailable ? icon : ''}</span>
            <span className="time-label">{isAvailable ? label : ''}</span>
          </button>
        );
      })}
    </div>
  );
};
