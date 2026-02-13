import type { FC, MouseEvent } from 'react';

interface VolumeSliderProps {
  value: number;
  onChange: (value: number) => void;
  onClick?: (e: MouseEvent) => void;
}

// 10 segments like Python's VolumeSlider
const SEGMENTS = 10;

// Colors: green at bottom (low volume) to red at top (high volume)
const getSegmentColor = (segmentIndex: number, isFilled: boolean): string => {
  if (!isFilled) {
    return 'rgba(50, 50, 50, 0.6)'; // Dark gray for empty
  }
  // Gradient from green (segment 0) to red (segment 9)
  // segmentIndex 0 = bottom = green, segmentIndex 9 = top = red
  const ratio = segmentIndex / (SEGMENTS - 1);
  const r = Math.round(50 + ratio * 205); // 50 -> 255
  const g = Math.round(200 - ratio * 150); // 200 -> 50
  const b = 50;
  return `rgb(${r}, ${g}, ${b})`;
};

export const VolumeSlider: FC<VolumeSliderProps> = ({ value, onChange, onClick }) => {
  // Calculate filled segments (value is 10-100, so divide by 10)
  const filledSegments = Math.round(value / 10);

  const handleClick = (e: MouseEvent<HTMLDivElement>) => {
    e.stopPropagation();
    e.preventDefault();
    onClick?.(e);

    // Calculate which segment was clicked based on Y position
    const rect = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const height = rect.height;

    // Invert Y (0 at bottom, SEGMENTS at top)
    const clickRatio = 1 - (y / height);
    const segment = Math.ceil(clickRatio * SEGMENTS);
    const newVolume = Math.max(10, Math.min(100, segment * 10));

    onChange(newVolume);
  };

  // Prevent drag events from bubbling
  const handleMouseDown = (e: MouseEvent<HTMLDivElement>) => {
    e.stopPropagation();
  };

  return (
    <div
      className="volume-slider-segmented"
      onClick={handleClick}
      onMouseDown={handleMouseDown}
      title={`Volume: ${value}%`}
    >
      <span className="volume-slider-icon">🔊</span>
      <div className="volume-slider-track">
        {/* Render segments from top (9) to bottom (0) */}
        {Array.from({ length: SEGMENTS }, (_, i) => {
          const segmentIndex = SEGMENTS - 1 - i; // Reverse so 9 is at top
          const isFilled = segmentIndex < filledSegments;
          return (
            <div
              key={segmentIndex}
              className={`volume-segment ${isFilled ? 'filled' : 'empty'}`}
              style={{ backgroundColor: getSegmentColor(segmentIndex, isFilled) }}
            />
          );
        })}
      </div>
    </div>
  );
};
