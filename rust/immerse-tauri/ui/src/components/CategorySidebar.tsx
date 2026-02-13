import { type FC, type MouseEvent, useRef, useState, useEffect, useCallback } from 'react';
import type { ActiveState, EnvironmentConfig } from '../types';

interface CategorySidebarProps {
  categories: string[];
  current: string;
  onSelect: (category: string) => void;
  activeState: ActiveState | null;
  allConfigs: Record<string, EnvironmentConfig[]>;
  soundCategories?: string[];
  onLightsBadgeClick?: (category: string, configName: string) => void;
  onAtmosphereBadgeClick?: (category: string, configNames: string[]) => void;
  /** When provided, a Settings item is shown at the bottom of the list (mobile only) */
  onSettingsClick?: () => void;
  /** When provided, a Debug Log item is shown below Settings (mobile only) */
  onDebugLogClick?: () => void;
}

interface OffScreenSoundsInfo {
  totalSounds: number;
  categoryCount: number;
  firstCategory: string | null;
}

// Category display names and icons
const categoryMeta: Record<string, { name: string; icon: string }> = {
  combat: { name: 'Combat', icon: '⚔️' },
  dungeon: { name: 'Dungeon', icon: '🏰' },
  nature: { name: 'Nature', icon: '🌲' },
  town: { name: 'Town', icon: '🏘️' },
  water: { name: 'Water', icon: '🌊' },
  celestial: { name: 'Celestial', icon: '✨' },
  spooky: { name: 'Spooky', icon: '👻' },
  relaxation: { name: 'Relaxation', icon: '🧘' },
  travel: { name: 'Travel', icon: '🧭' },
  weather: { name: 'Weather', icon: '⛈️' },
  special: { name: 'Special', icon: '🎭' },
};

// Helper to check if a category has active lights and return the config name + icon
function getCategoryWithActiveLights(
  activeState: ActiveState | null,
  allConfigs: Record<string, EnvironmentConfig[]>
): { category: string; configName: string; configIcon?: string } | null {
  if (!activeState?.active_lights_config) return null;
  for (const [category, configs] of Object.entries(allConfigs)) {
    const config = configs.find(c => c.name === activeState.active_lights_config);
    if (config) {
      return { category, configName: activeState.active_lights_config, configIcon: config.icon };
    }
  }
  return null;
}

// Helper to get count of active atmosphere sounds in a category and their config names
function getActiveAtmosphereInfo(
  category: string,
  activeState: ActiveState | null,
  allConfigs: Record<string, EnvironmentConfig[]>
): { count: number; configNames: string[] } {
  if (!activeState || activeState.active_atmosphere_urls.length === 0) {
    return { count: 0, configNames: [] };
  }
  const configs = allConfigs[category] || [];
  const configNames: string[] = [];
  for (const config of configs) {
    // Check loop sounds (sound file URL in active atmosphere)
    const soundUrl = config.engines?.sound?.file;
    if (soundUrl && activeState.active_atmosphere_urls.includes(soundUrl)) {
      configNames.push(config.name);
      continue;
    }
    // Check atmosphere mix URLs (for full environments like circus, river)
    const mix = config.engines?.atmosphere?.mix;
    if (mix && mix.length > 0) {
      const hasActiveAtmosphere = mix.some(m =>
        activeState.active_atmosphere_urls.includes(m.url)
      );
      if (hasActiveAtmosphere) {
        configNames.push(config.name);
        continue;
      }
    }
    // Fallback: if this config is the active lights config and has atmosphere enabled,
    // treat it as having active atmosphere (covers URL serialization edge cases)
    if (activeState.active_lights_config === config.name &&
        config.engines?.atmosphere?.enabled) {
      configNames.push(config.name);
    }
  }
  return { count: configNames.length, configNames };
}

export const CategorySidebar: FC<CategorySidebarProps> = ({
  categories,
  current,
  onSelect,
  activeState,
  allConfigs,
  soundCategories = [],
  onLightsBadgeClick,
  onAtmosphereBadgeClick,
  onSettingsClick,
  onDebugLogClick,
}) => {
  const listRef = useRef<HTMLUListElement>(null);
  const sidebarRef = useRef<HTMLElement>(null);
  const [offScreenSounds, setOffScreenSounds] = useState<OffScreenSoundsInfo>({
    totalSounds: 0,
    categoryCount: 0,
    firstCategory: null,
  });

  const activeLightsInfo = getCategoryWithActiveLights(activeState, allConfigs);

  // Separate environment categories from sound categories
  const envCategories = categories.filter(c => !soundCategories.includes(c));
  const soundCats = categories.filter(c => soundCategories.includes(c));

  // Build a map of category -> atmosphere count for off-screen calculation
  const getCategoryAtmosphereCounts = useCallback(() => {
    const counts: Record<string, number> = {};
    for (const category of categories) {
      const info = getActiveAtmosphereInfo(category, activeState, allConfigs);
      if (info.count > 0) {
        counts[category] = info.count;
      }
    }
    return counts;
  }, [categories, activeState, allConfigs]);

  // Calculate which categories with active sounds are off-screen (below visible area)
  const updateOffScreenSounds = useCallback(() => {
    const sidebar = sidebarRef.current;
    const list = listRef.current;
    if (!sidebar || !list) return;

    const sidebarRect = sidebar.getBoundingClientRect();
    const viewportBottom = sidebarRect.bottom;
    const counts = getCategoryAtmosphereCounts();

    let totalSounds = 0;
    let categoryCount = 0;
    let firstCategory: string | null = null;

    // Find all category items and check if they're below the visible area
    const items = list.querySelectorAll('[data-category]');
    items.forEach(item => {
      const category = item.getAttribute('data-category');
      if (!category || !counts[category]) return;

      const itemRect = item.getBoundingClientRect();
      // Item is below the visible area if its top is >= viewport bottom
      if (itemRect.top >= viewportBottom) {
        totalSounds += counts[category];
        categoryCount++;
        if (!firstCategory) {
          firstCategory = category;
        }
      }
    });

    setOffScreenSounds({ totalSounds, categoryCount, firstCategory });
  }, [getCategoryAtmosphereCounts]);

  // Update off-screen sounds on scroll and resize
  useEffect(() => {
    const sidebar = sidebarRef.current;
    if (!sidebar) return;

    // Initial calculation
    updateOffScreenSounds();

    // Listen to scroll events on the sidebar
    sidebar.addEventListener('scroll', updateOffScreenSounds);

    // Also listen to window resize
    window.addEventListener('resize', updateOffScreenSounds);

    return () => {
      sidebar.removeEventListener('scroll', updateOffScreenSounds);
      window.removeEventListener('resize', updateOffScreenSounds);
    };
  }, [updateOffScreenSounds]);

  // Re-calculate when active state changes
  useEffect(() => {
    updateOffScreenSounds();
  }, [activeState, updateOffScreenSounds]);

  const handleFloatingBadgeClick = () => {
    if (offScreenSounds.firstCategory && listRef.current) {
      const item = listRef.current.querySelector(`[data-category="${offScreenSounds.firstCategory}"]`);
      if (item) {
        item.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    }
  };

  // Generate badge text
  const getBadgeText = () => {
    const { totalSounds, categoryCount } = offScreenSounds;
    if (categoryCount === 1) {
      return `🔊 ${totalSounds} sound${totalSounds > 1 ? 's' : ''} below ↓`;
    }
    return `🔊 ${totalSounds} sound${totalSounds > 1 ? 's' : ''} in ${categoryCount} categories ↓`;
  };

  const renderCategory = (category: string) => {
    const meta = categoryMeta[category] || { name: category, icon: '📁' };
    const hasActiveLights = activeLightsInfo?.category === category;
    const atmosphereInfo = getActiveAtmosphereInfo(category, activeState, allConfigs);

    const handleLightsBadgeClick = (e: MouseEvent) => {
      e.stopPropagation();
      if (activeLightsInfo && onLightsBadgeClick) {
        onLightsBadgeClick(category, activeLightsInfo.configName);
      }
    };

    const handleAtmosphereBadgeClick = (e: MouseEvent) => {
      e.stopPropagation();
      if (atmosphereInfo.configNames.length > 0 && onAtmosphereBadgeClick) {
        onAtmosphereBadgeClick(category, atmosphereInfo.configNames);
      }
    };

    return (
      <li key={category} data-category={category}>
        <div
          role="button"
          tabIndex={0}
          className={`category-item ${current === category ? 'active' : ''}`}
          onClick={() => onSelect(category)}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') onSelect(category); }}
          title={meta.name}
        >
          <span className="category-icon">{meta.icon}</span>
          <span className="category-name">{meta.name}</span>
          <span className="category-badges">
            {hasActiveLights && (
              <span
                role="button"
                tabIndex={0}
                className="category-badge lights clickable"
                title={`Now playing: ${activeLightsInfo!.configName}`}
                onClick={handleLightsBadgeClick}
                onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleLightsBadgeClick(e as unknown as MouseEvent); }}
              >
                {activeLightsInfo!.configIcon || '💡'}
              </span>
            )}
            {atmosphereInfo.count > 0 && (
              <span
                role="button"
                tabIndex={0}
                className="category-badge atmosphere clickable"
                title={`${atmosphereInfo.count} sound(s) playing - click to show`}
                onClick={handleAtmosphereBadgeClick}
                onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleAtmosphereBadgeClick(e as unknown as MouseEvent); }}
              >
                🔊{atmosphereInfo.count > 1 ? atmosphereInfo.count : ''}
              </span>
            )}
          </span>
        </div>
      </li>
    );
  };

  return (
    <nav className="category-sidebar" ref={sidebarRef}>
      <div className="sidebar-header">
        <h2>Categories</h2>
      </div>
      <ul className="category-list" ref={listRef}>
        {envCategories.map(renderCategory)}
        {soundCats.length > 0 && (
          <>
            <li className="category-separator">
              <span>── SOUNDS ──</span>
            </li>
            {soundCats.map(renderCategory)}
          </>
        )}
        {(onSettingsClick || onDebugLogClick) && (
          <>
            <li className="category-separator settings-separator">
              <span></span>
            </li>
            {onSettingsClick && (
              <li>
                <div
                  role="button"
                  tabIndex={0}
                  className="category-item settings-item"
                  onClick={onSettingsClick}
                  onKeyDown={(e) => { if ((e.key === 'Enter' || e.key === ' ') && onSettingsClick) onSettingsClick(); }}
                  title="Settings"
                >
                  <span className="category-icon">&#x2699;&#xFE0F;</span>
                  <span className="category-name">Settings</span>
                </div>
              </li>
            )}
            {onDebugLogClick && (
              <li>
                <div
                  role="button"
                  tabIndex={0}
                  className="category-item settings-item"
                  onClick={onDebugLogClick}
                  onKeyDown={(e) => { if ((e.key === 'Enter' || e.key === ' ') && onDebugLogClick) onDebugLogClick(); }}
                  title="Debug Log"
                >
                  <span className="category-icon">&#x1F41B;</span>
                  <span className="category-name">Debug Log</span>
                </div>
              </li>
            )}
          </>
        )}
      </ul>
      {offScreenSounds.totalSounds > 0 && (
        <button
          className="floating-sounds-badge"
          onClick={handleFloatingBadgeClick}
          title="Click to scroll to active sounds"
        >
          {getBadgeText()}
        </button>
      )}
    </nav>
  );
};
