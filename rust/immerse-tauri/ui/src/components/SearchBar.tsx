import type { FC, ChangeEvent, KeyboardEvent } from 'react';
import { useRef, useEffect } from 'react';

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  onSelect?: () => void;
}

export const SearchBar: FC<SearchBarProps> = ({ value, onChange, onSelect }) => {
  const inputRef = useRef<HTMLInputElement>(null);

  // Handle Ctrl+L to focus search
  useEffect(() => {
    const handleKeyDown = (e: globalThis.KeyboardEvent) => {
      if (e.ctrlKey && e.key === 'l') {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    onChange(e.target.value);
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      onChange('');
      inputRef.current?.blur();
    } else if (e.key === 'Enter' && onSelect) {
      e.stopPropagation();
      onSelect();
    }
  };

  return (
    <div className="search-bar">
      <span className="search-icon">🔍</span>
      <input
        ref={inputRef}
        type="text"
        placeholder="Search environments... (Ctrl+L)"
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        className="search-input"
      />
      {value && (
        <button
          className="search-clear"
          onClick={() => onChange('')}
          title="Clear search"
        >
          ✕
        </button>
      )}
    </div>
  );
};
