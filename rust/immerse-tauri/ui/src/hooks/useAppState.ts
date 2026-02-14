import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentConfig, ActiveState, AvailableTimes } from '../types';

export function useAppState() {
  const [categories, setCategories] = useState<string[]>([]);
  const [currentCategory, setCurrentCategory] = useState('');
  const [environments, setEnvironments] = useState<EnvironmentConfig[]>([]);
  const [allConfigs, setAllConfigs] = useState<Record<string, EnvironmentConfig[]>>({});
  const [activeState, setActiveState] = useState<ActiveState | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<EnvironmentConfig[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isStartingEnvironment, setIsStartingEnvironment] = useState(false);
  const [soundCategories, setSoundCategories] = useState<string[]>([]);

  // Load initial data and trigger startup environment
  useEffect(() => {
    async function loadInitialData() {
      try {
        const [cats, configs, state, soundCats] = await Promise.all([
          invoke<string[]>('get_categories'),
          invoke<Record<string, EnvironmentConfig[]>>('get_all_configs'),
          invoke<ActiveState>('get_active_state'),
          invoke<string[]>('get_sound_categories'),
        ]);

        setCategories(cats);
        setAllConfigs(configs);
        setActiveState(state);
        setSoundCategories(soundCats);

        if (cats.length > 0) {
          setCurrentCategory(cats[0]);
          setEnvironments(configs[cats[0]] || []);
        }

        // Trigger the startup environment (hidden "Startup" config)
        // This matches Python's behavior in gui/controller/startup.py
        try {
          const startupEnv = await invoke<string | null>('trigger_startup');
          if (startupEnv) {
            console.log(`Triggered startup environment: ${startupEnv}`);
            // Refresh state after startup
            const newState = await invoke<ActiveState>('get_active_state');
            setActiveState(newState);
          }
        } catch (startupErr) {
          console.error('Failed to trigger startup environment:', startupErr);
        }
      } catch (err) {
        console.error('Failed to load initial data:', err);
      } finally {
        setIsLoading(false);
      }
    }

    loadInitialData();
  }, []);

  // Load environments when category changes
  useEffect(() => {
    if (currentCategory && allConfigs[currentCategory]) {
      setEnvironments(allConfigs[currentCategory]);
    }
  }, [currentCategory, allConfigs]);

  // Refresh active state periodically
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const state = await invoke<ActiveState>('get_active_state');
        setActiveState(state);
      } catch (err) {
        console.error('Failed to refresh state:', err);
      }
    }, 1000);

    return () => clearInterval(interval);
  }, []);

  // Search handler
  const handleSearch = useCallback(async (query: string) => {
    setSearchQuery(query);
    if (query.trim() === '') {
      setSearchResults([]);
      return;
    }
    try {
      const results = await invoke<EnvironmentConfig[]>('search_configs', { query });
      setSearchResults(results);
    } catch (err) {
      console.error('Search failed:', err);
      setSearchResults([]);
    }
  }, []);

  // Start environment
  const startEnvironment = useCallback(async (configName: string) => {
    setIsStartingEnvironment(true);
    try {
      await invoke('start_environment', { configName });
      // Refresh state
      const state = await invoke<ActiveState>('get_active_state');
      setActiveState(state);
    } catch (err) {
      console.error('Failed to start environment:', err);
    } finally {
      setIsStartingEnvironment(false);
    }
  }, []);

  // Start environment with specific time variant
  const startEnvironmentWithTime = useCallback(async (configName: string, time: string) => {
    console.log('[BACKEND] startEnvironmentWithTime called:', configName, time);
    setIsStartingEnvironment(true);
    try {
      await invoke('start_environment_with_time', { configName, time });
      console.log('[BACKEND] start_environment_with_time invoke completed');
      // Refresh state
      const state = await invoke<ActiveState>('get_active_state');
      console.log('[BACKEND] New active state:', state.active_lights_config, 'atmosphere:', state.active_atmosphere_urls.length);
      setActiveState(state);
    } catch (err) {
      console.error('Failed to start environment with time:', err);
    } finally {
      setIsStartingEnvironment(false);
    }
  }, []);

  // Get available times for a config
  const getAvailableTimes = useCallback(async (configName: string): Promise<AvailableTimes> => {
    try {
      return await invoke<AvailableTimes>('get_available_times', { configName });
    } catch (err) {
      console.error('Failed to get available times:', err);
      return { config_name: configName, times: [], has_variants: false };
    }
  }, []);

  // Set current time
  const setCurrentTime = useCallback(async (time: string) => {
    try {
      await invoke('set_current_time', { time });
    } catch (err) {
      console.error('Failed to set current time:', err);
    }
  }, []);

  // Toggle loop sound
  const toggleLoopSound = useCallback(async (url: string) => {
    try {
      await invoke('toggle_loop_sound', { url });
      // Refresh state
      const state = await invoke<ActiveState>('get_active_state');
      setActiveState(state);
    } catch (err) {
      console.error('Failed to toggle loop sound:', err);
    }
  }, []);

  // Set volume
  const setVolume = useCallback(async (url: string, volume: number) => {
    try {
      await invoke('set_volume', { url, volume });
      // Update local state optimistically
      setActiveState(prev => prev ? {
        ...prev,
        atmosphere_volumes: { ...prev.atmosphere_volumes, [url]: volume }
      } : null);
    } catch (err) {
      console.error('Failed to set volume:', err);
    }
  }, []);

  // Stop lights
  const stopLights = useCallback(async () => {
    try {
      await invoke('stop_lights');
      const state = await invoke<ActiveState>('get_active_state');
      setActiveState(state);
    } catch (err) {
      console.error('Failed to stop lights:', err);
    }
  }, []);

  // Stop sounds
  const stopSounds = useCallback(async () => {
    try {
      const count = await invoke<number>('stop_sounds');
      console.log(`Stopped ${count} sounds`);
    } catch (err) {
      console.error('Failed to stop sounds:', err);
    }
  }, []);

  // Stop atmosphere
  const stopAtmosphere = useCallback(async () => {
    try {
      await invoke('stop_atmosphere');
      const state = await invoke<ActiveState>('get_active_state');
      setActiveState(state);
    } catch (err) {
      console.error('Failed to stop atmosphere:', err);
    }
  }, []);

  // Toggle pause/resume all sounds
  const togglePauseSounds = useCallback(async () => {
    try {
      await invoke<boolean>('toggle_pause_sounds');
      const state = await invoke<ActiveState>('get_active_state');
      setActiveState(state);
    } catch (err) {
      console.error('Failed to toggle pause sounds:', err);
    }
  }, []);

  return {
    // Data
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

    // Setters
    setCurrentCategory,

    // Actions
    handleSearch,
    startEnvironment,
    startEnvironmentWithTime,
    getAvailableTimes,
    setCurrentTime,
    toggleLoopSound,
    setVolume,
    stopLights,
    stopSounds,
    stopAtmosphere,
    togglePauseSounds,
  };
}
