import { useState, useEffect, useCallback, useRef, useSyncExternalStore } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { CategorySidebar } from './components/CategorySidebar';
import { EnvironmentGrid } from './components/EnvironmentGrid';
import { StopButtons } from './components/StopButtons';
import { TopBar } from './components/TopBar';
import { StatusBar } from './components/StatusBar';
import { TimeVariantDialog } from './components/TimeVariantDialog';
import { SettingsDialog } from './components/SettingsDialog';
import { DebugLogPanel } from './components/DebugLogPanel';
import { ThemeProvider } from './contexts/ThemeContext';
import { useAppState } from './hooks/useAppState';
import type { EnvironmentConfig, AvailableTimes } from './types';

// Subscribe to media query changes for responsive layout
const mobileQuery = typeof window !== 'undefined' ? window.matchMedia('(max-width: 960px)') : null;
function subscribeToMediaQuery(callback: () => void) {
  mobileQuery?.addEventListener('change', callback);
  return () => mobileQuery?.removeEventListener('change', callback);
}

function App() {
  return (
    <ThemeProvider>
      <AppContent />
    </ThemeProvider>
  );
}

function AppContent() {
  const {
    categories,
    currentCategory,
    environments,
    allConfigs,
    activeState,
    searchQuery,
    searchResults,
    isLoading,
    isStartingEnvironment,
    soundCategories,
    setCurrentCategory,
    handleSearch,
    startEnvironment,
    startEnvironmentWithTime,
    getAvailableTimes,
    setCurrentTime: setBackendTime,
    toggleLoopSound,
    setVolume,
    stopLights,
    togglePauseSounds,
  } = useAppState();

  // Responsive: detect tablet/mobile width
  const isMobileMode = useSyncExternalStore(
    subscribeToMediaQuery,
    () => window.matchMedia('(max-width: 960px)').matches,
    () => false, // SSR fallback
  );
  const [showMobileCategories, setShowMobileCategories] = useState(false);

  // Close mobile categories when switching back to desktop
  useEffect(() => {
    if (!isMobileMode) setShowMobileCategories(false);
  }, [isMobileMode]);

  // Swipe gestures for mobile hamburger menu
  const touchStartRef = useRef<{ x: number; y: number; time: number } | null>(null);
  useEffect(() => {
    if (!isMobileMode) return;

    const handleTouchStart = (e: TouchEvent) => {
      const touch = e.touches[0];
      touchStartRef.current = { x: touch.clientX, y: touch.clientY, time: Date.now() };
    };

    const handleTouchEnd = (e: TouchEvent) => {
      if (!touchStartRef.current) return;
      const touch = e.changedTouches[0];
      const dx = touch.clientX - touchStartRef.current.x;
      const dy = touch.clientY - touchStartRef.current.y;
      const elapsed = Date.now() - touchStartRef.current.time;
      const startX = touchStartRef.current.x;
      touchStartRef.current = null;

      // Must be a quick horizontal swipe (>60px horizontal, <40px vertical, <400ms)
      if (Math.abs(dx) < 60 || Math.abs(dy) > 40 || elapsed > 400) return;

      const screenWidth = window.innerWidth;

      if (dx > 0 && startX < screenWidth / 3) {
        // Swipe right on left 1/3 -> open menu
        setShowMobileCategories(true);
      } else if (dx < 0 && startX > screenWidth * 2 / 3) {
        // Swipe left on right 1/3 -> close menu
        setShowMobileCategories(false);
      }
    };

    window.addEventListener('touchstart', handleTouchStart, { passive: true });
    window.addEventListener('touchend', handleTouchEnd, { passive: true });
    return () => {
      window.removeEventListener('touchstart', handleTouchStart);
      window.removeEventListener('touchend', handleTouchEnd);
    };
  }, [isMobileMode]);

  // Time of day state - default is "daytime" (base config, no time variant overrides)
  const [currentTime, setCurrentTime] = useState('daytime');

  // Available times come from the active environment's time variants
  // Empty array means no lights config is active OR config has no time variants
  // (all time buttons should be blank/disabled)
  // Non-empty array means those specific times are available for the current config
  const availableTimes = activeState?.available_times ?? [];

  // Time variant dialog state
  const [timeDialogOpen, setTimeDialogOpen] = useState(false);
  const [pendingConfig, setPendingConfig] = useState<EnvironmentConfig | null>(null);
  const [pendingTimes, setPendingTimes] = useState<AvailableTimes | null>(null);

  // Settings dialog state
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Debug log dialog state
  const [debugLogOpen, setDebugLogOpen] = useState(false);

  // Search focus state (for Enter key behavior)
  const [searchFocusedIndex, setSearchFocusedIndex] = useState<number | null>(null);
  const searchFocusedAtRef = useRef<number>(0);

  // Sync current time with backend
  useEffect(() => {
    if (activeState?.current_time) {
      setCurrentTime(activeState.current_time);
    }
  }, [activeState?.current_time]);

  // Expose settings opener to window for Rust menu to call via eval
  useEffect(() => {
    (window as unknown as { __openSettings: () => void }).__openSettings = () => {
      setSettingsOpen(true);
    };

    return () => {
      delete (window as unknown as { __openSettings?: () => void }).__openSettings;
    };
  }, []);


  // Keyboard shortcuts for time of day (1-4) and global shortcuts
  // Disabled on mobile - no physical keyboard
  useEffect(() => {
    if (isMobileMode) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Global shortcuts that work even when input is focused
      // Ctrl+Q to quit
      if (e.ctrlKey && e.key === 'q') {
        e.preventDefault();
        getCurrentWindow().close();
        return;
      }

      // Ctrl+, to open settings
      if (e.ctrlKey && e.key === ',') {
        e.preventDefault();
        setSettingsOpen(true);
        return;
      }

      if (isInputFocused()) return;

      // Escape clears search query (when search input is not focused)
      if (e.key === 'Escape' && searchQuery) {
        handleSearch('');
        return;
      }

      // Enter activates the search-focused button (after first Enter blurred the input)
      // Timing guard prevents key repeat from immediately activating
      if (e.key === 'Enter' && searchFocusedIndex !== null && Date.now() - searchFocusedAtRef.current > 300) {
        const envs = searchQuery ? searchResults : environments;
        const config = envs[searchFocusedIndex];
        if (config) {
          const isLoop = config.metadata?.loop || config.engines.sound?.loop;
          if (isLoop) {
            const url = config.engines.sound?.file ?? '';
            if (url) toggleLoopSound(url);
          } else {
            handleEnvironmentClick(config);
          }
          setSearchFocusedIndex(null);
        }
        return;
      }

      // Skip time shortcuts if the time variant dialog is open - it has its own handler
      if (timeDialogOpen) return;

      // Time of day shortcuts (1-4) - matches Python's TIME_PERIODS
      // Only works when availableTimes is non-empty AND includes the specific time
      const timeKeys: Record<string, string> = {
        '1': 'morning',
        '2': 'daytime',
        '3': 'afternoon',
        '4': 'evening',
      };

      if (e.key in timeKeys && !e.ctrlKey && !e.altKey && !e.metaKey) {
        const time = timeKeys[e.key];
        // Shortcuts only work when there are time variants available
        if (availableTimes.length > 0 && availableTimes.includes(time)) {
          handleTimeChange(time);
        }
        return;
      }

      // "5" shortcut to navigate to active lights config
      if (e.key === '5' && !e.ctrlKey && !e.altKey && !e.metaKey) {
        if (activeState?.active_lights_config) {
          // Find the category containing the active lights config and navigate to it
          for (const [category, configs] of Object.entries(allConfigs)) {
            if (configs.some(c => c.name === activeState.active_lights_config)) {
              setCurrentCategory(category);
              setHighlightedConfig(activeState.active_lights_config);
              setTimeout(() => setHighlightedConfig(null), 3000);
              break;
            }
          }
        }
        return;
      }

      // Ctrl+PgUp/PgDn for category navigation
      if (e.ctrlKey && (e.key === 'PageUp' || e.key === 'PageDown')) {
        e.preventDefault();
        const currentIndex = categories.indexOf(currentCategory);
        if (currentIndex >= 0) {
          const newIndex = e.key === 'PageUp'
            ? Math.max(0, currentIndex - 1)
            : Math.min(categories.length - 1, currentIndex + 1);
          setCurrentCategory(categories[newIndex]);
        }
        return;
      }

      // Letter shortcuts for environments (Q, W, E, R, T, Y, U, I, O, P, A, S, D, F, G, H, J, K, L)
      const shortcuts = 'QWERTYUIOPASDFGHJKL';
      const upperKey = e.key.toUpperCase();
      const shortcutIndex = shortcuts.indexOf(upperKey);
      if (shortcutIndex >= 0 && !e.ctrlKey && !e.altKey && !e.metaKey) {
        const envs = searchQuery ? searchResults : environments;
        if (shortcutIndex < envs.length) {
          const config = envs[shortcutIndex];
          const isLoop = config.metadata?.loop || config.engines.sound?.loop;
          if (isLoop) {
            const url = config.engines.sound?.file ?? '';
            if (url) toggleLoopSound(url);
          } else {
            handleEnvironmentClick(config);
          }
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isMobileMode, categories, currentCategory, environments, searchQuery, searchResults, availableTimes, setCurrentCategory, toggleLoopSound, timeDialogOpen, activeState?.active_lights_config, allConfigs, handleSearch, searchFocusedIndex]);

  const displayedEnvironments = searchQuery ? searchResults : environments;

  // Clear search focus when query changes
  useEffect(() => {
    setSearchFocusedIndex(null);
  }, [searchQuery]);

  const handleVolumeChange = useCallback((url: string, volume: number) => {
    setVolume(url, volume);
  }, [setVolume]);

  const handleTimeChange = useCallback(async (time: string) => {
    setCurrentTime(time);
    await setBackendTime(time);

    // If we have an active lights config, restart it with the new time variant
    if (activeState?.active_lights_config) {
      await startEnvironmentWithTime(activeState.active_lights_config, time);
    }
  }, [setBackendTime, activeState?.active_lights_config, startEnvironmentWithTime]);

  // Handle environment click - check for time variants
  const handleEnvironmentClick = useCallback(async (config: EnvironmentConfig) => {
    console.log('[ENV_CLICK] handleEnvironmentClick called with config:', config.name, 'category:', config.category);
    const times = await getAvailableTimes(config.name);
    console.log('[ENV_CLICK] getAvailableTimes returned:', times);

    if (times.has_variants && times.times.length > 1) {
      // Show time variant dialog
      console.log('[ENV_CLICK] Showing time variant dialog for:', config.name);
      setPendingConfig(config);
      setPendingTimes(times);
      setTimeDialogOpen(true);
    } else if (times.has_variants && times.times.length === 1) {
      // Only one variant, use it directly
      console.log('[ENV_CLICK] Single variant, calling startEnvironmentWithTime:', config.name, times.times[0]);
      await startEnvironmentWithTime(config.name, times.times[0]);
    } else {
      // No variants, start normally
      console.log('[ENV_CLICK] No variants, calling startEnvironment:', config.name);
      await startEnvironment(config.name);
    }
  }, [getAvailableTimes, startEnvironment, startEnvironmentWithTime]);

  // Handle search Enter key - first Enter focuses, second Enter activates
  const handleSearchSelect = useCallback(() => {
    const envs = searchQuery ? searchResults : environments;
    if (envs.length === 0) return;

    if (searchFocusedIndex === null) {
      // First Enter: focus the first result and blur search input
      // so keyboard shortcuts (including Enter for activation) work
      setSearchFocusedIndex(0);
      searchFocusedAtRef.current = Date.now();
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
    } else {
      // Second Enter: activate the focused button
      const config = envs[searchFocusedIndex];
      if (config) {
        const isLoop = config.metadata?.loop || config.engines.sound?.loop;
        if (isLoop) {
          const url = config.engines.sound?.file ?? '';
          if (url) toggleLoopSound(url);
        } else {
          handleEnvironmentClick(config);
        }
        // Clear focus after activation
        setSearchFocusedIndex(null);
      }
    }
  }, [searchQuery, searchResults, environments, searchFocusedIndex, toggleLoopSound, handleEnvironmentClick]);

  // Handle time variant selection from dialog
  const handleTimeVariantSelect = useCallback(async (time: string) => {
    console.log('[TIME_SELECT] handleTimeVariantSelect called with time:', time, 'pendingConfig:', pendingConfig?.name);
    const config = pendingConfig;  // Capture before clearing state
    setTimeDialogOpen(false);
    setPendingConfig(null);
    setPendingTimes(null);

    if (config) {
      console.log('[TIME_SELECT] Calling startEnvironmentWithTime:', config.name, time);
      setCurrentTime(time);
      await setBackendTime(time);
      await startEnvironmentWithTime(config.name, time);
      console.log('[TIME_SELECT] startEnvironmentWithTime completed');
    } else {
      console.log('[TIME_SELECT] ERROR: pendingConfig is null!');
    }
  }, [pendingConfig, startEnvironmentWithTime, setBackendTime]);

  const handleTimeDialogClose = useCallback(() => {
    setTimeDialogOpen(false);
    setPendingConfig(null);
    setPendingTimes(null);
  }, []);

  // Track which config to highlight/scroll to
  const [highlightedConfig, setHighlightedConfig] = useState<string | null>(null);

  const handleLightsClick = useCallback(() => {
    // Navigate to the category containing the active lights config
    if (activeState?.active_lights_config) {
      for (const [category, configs] of Object.entries(allConfigs)) {
        if (configs.some(c => c.name === activeState.active_lights_config)) {
          setCurrentCategory(category);
          setHighlightedConfig(activeState.active_lights_config);
          // Clear highlight after animation
          setTimeout(() => setHighlightedConfig(null), 3000);
          break;
        }
      }
    }
  }, [activeState, allConfigs, setCurrentCategory]);

  // Handle lights badge click in category sidebar
  const handleCategoryLightsBadgeClick = useCallback((category: string, configName: string) => {
    setCurrentCategory(category);
    setHighlightedConfig(configName);
    // Clear highlight after animation
    setTimeout(() => setHighlightedConfig(null), 3000);
  }, [setCurrentCategory]);

  // Handle atmosphere badge click in category sidebar
  const handleCategoryAtmosphereBadgeClick = useCallback((category: string, configNames: string[]) => {
    setCurrentCategory(category);
    if (configNames.length > 0) {
      setHighlightedConfig(configNames[0]);
      // Clear highlight after animation
      setTimeout(() => setHighlightedConfig(null), 3000);
    }
  }, [setCurrentCategory]);

  // Select category and clear any active search
  const handleCategorySelect = useCallback((category: string) => {
    if (searchQuery) {
      handleSearch('');
    }
    setCurrentCategory(category);
  }, [setCurrentCategory, searchQuery, handleSearch]);

  // Mobile: select category and close the mobile menu
  const handleMobileCategorySelect = useCallback((category: string) => {
    if (searchQuery) {
      handleSearch('');
    }
    setCurrentCategory(category);
    setShowMobileCategories(false);
  }, [setCurrentCategory, searchQuery, handleSearch]);

  if (isLoading) {
    return (
      <div className="app loading">
        <div className="loading-spinner">Loading...</div>
      </div>
    );
  }

  return (
    <div className={`app${isMobileMode ? ' mobile-mode' : ''}`}>
      <header className="top-bar-container">
        <TopBar
          searchQuery={searchQuery}
          onSearchChange={handleSearch}
          onSearchSelect={handleSearchSelect}
          currentTime={currentTime}
          availableTimes={availableTimes}
          onTimeChange={handleTimeChange}
          activeState={activeState}
          allConfigs={allConfigs}
          onLightsClick={handleLightsClick}
          showHamburger={isMobileMode}
          mobileMenuOpen={showMobileCategories}
          onHamburgerClick={() => setShowMobileCategories(v => !v)}
        />
      </header>

      {!isMobileMode && (
        <aside className="sidebar">
          <CategorySidebar
            categories={categories}
            current={currentCategory}
            onSelect={handleCategorySelect}
            activeState={activeState}
            allConfigs={allConfigs}
            soundCategories={soundCategories}
            onLightsBadgeClick={handleCategoryLightsBadgeClick}
            onAtmosphereBadgeClick={handleCategoryAtmosphereBadgeClick}
          />
        </aside>
      )}

      <main className="content">
        {isMobileMode && showMobileCategories ? (
          <div className="mobile-categories">
            <CategorySidebar
              categories={categories}
              current={currentCategory}
              onSelect={handleMobileCategorySelect}
              activeState={activeState}
              allConfigs={allConfigs}
              soundCategories={soundCategories}
              onLightsBadgeClick={handleCategoryLightsBadgeClick}
              onAtmosphereBadgeClick={handleCategoryAtmosphereBadgeClick}
              onSettingsClick={() => { setSettingsOpen(true); setShowMobileCategories(false); }}
              onDebugLogClick={() => { setDebugLogOpen(true); setShowMobileCategories(false); }}
            />
          </div>
        ) : (
          <>
            <header className="content-header">
              <h1>{searchQuery ? `Search: "${searchQuery}"` : currentCategory}</h1>
            </header>

            <EnvironmentGrid
              environments={displayedEnvironments}
              activeState={activeState}
              focusedIndex={searchQuery ? searchFocusedIndex : null}
              highlightedConfig={highlightedConfig}
              onStartEnvironment={handleEnvironmentClick}
              onToggleLoop={toggleLoopSound}
              onVolumeChange={handleVolumeChange}
            />
          </>
        )}
      </main>

      <footer className="controls">
        <StopButtons
          onStopLights={stopLights}
          onTogglePause={togglePauseSounds}
          lightsActive={!!activeState?.active_lights_config}
          soundsActive={(activeState?.active_atmosphere_urls.length ?? 0) > 0 || !!activeState?.active_sound}
          isPaused={activeState?.is_sounds_paused ?? false}
          isMobileMode={isMobileMode}
        />
      </footer>

      <StatusBar activeState={activeState} />

      {isStartingEnvironment && (
        <div className="loading-toast">Loading environment...</div>
      )}

      {timeDialogOpen && pendingTimes && (
        <TimeVariantDialog
          configName={pendingConfig?.name ?? ''}
          availableTimes={pendingTimes.times}
          currentTime={currentTime}
          onSelect={handleTimeVariantSelect}
          onClose={handleTimeDialogClose}
        />
      )}

      {settingsOpen && (
        <SettingsDialog onClose={() => setSettingsOpen(false)} />
      )}

      {debugLogOpen && (
        <DebugLogPanel onClose={() => setDebugLogOpen(false)} />
      )}
    </div>
  );
}

// Helper to check if an input is focused
function isInputFocused(): boolean {
  const active = document.activeElement;
  return active instanceof HTMLInputElement ||
         active instanceof HTMLTextAreaElement ||
         active instanceof HTMLSelectElement;
}

export default App;
