import { useState, useEffect, type FC } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-shell';
import { useTheme, type Theme } from '../contexts/ThemeContext';

interface SettingsDialogProps {
  onClose: () => void;
}

type SettingsPanel = 'appearance' | 'spotify' | 'bulbs' | 'downloads' | 'user_content';

// Types for backend configs
interface SpotifyConfig {
  username: string;
  client_id: string;
  client_secret: string;
  redirect_uri: string;
  auto_start: string;
  is_configured: boolean;
}

interface WizBulbConfig {
  backdrop_bulbs: string;
  overhead_bulbs: string;
  battlefield_bulbs: string;
  is_configured: boolean;
}

interface AppSettings {
  ignore_ssl_errors: boolean;
  spotify_auto_start: string;
  downloads_enabled: boolean;
}

const PANELS: { id: SettingsPanel; icon: string; label: string }[] = [
  { id: 'appearance', icon: '🎨', label: 'Appearance' },
  { id: 'spotify', icon: '🎵', label: 'Spotify' },
  { id: 'bulbs', icon: '💡', label: 'WIZ Bulbs' },
  { id: 'downloads', icon: '📥', label: 'Downloads' },
  { id: 'user_content', icon: '📁', label: 'User Content' },
];

export const SettingsDialog: FC<SettingsDialogProps> = ({ onClose }) => {
  const [activePanel, setActivePanel] = useState<SettingsPanel>('appearance');
  const [spotifyConfig, setSpotifyConfig] = useState<SpotifyConfig | null>(null);
  const [bulbConfig, setBulbConfig] = useState<WizBulbConfig | null>(null);
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);

  // Load all configs on mount
  useEffect(() => {
    const loadConfigs = async () => {
      try {
        const [spotify, bulbs, settings] = await Promise.all([
          invoke<SpotifyConfig>('get_spotify_config'),
          invoke<WizBulbConfig>('get_wizbulb_config'),
          invoke<AppSettings>('get_app_settings'),
        ]);
        setSpotifyConfig(spotify);
        setBulbConfig(bulbs);
        setAppSettings(settings);
      } catch (e) {
        console.error('Failed to load settings:', e);
      }
    };
    loadConfigs();
  }, []);

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  // Get status indicator for nav items
  const getNavStatus = (panel: SettingsPanel): string => {
    switch (panel) {
      case 'spotify':
        return spotifyConfig?.is_configured ? '✓' : '!';
      case 'bulbs':
        return bulbConfig?.is_configured ? '✓' : '!';
      default:
        return '';
    }
  };

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        className="dialog-content settings-dialog"
        onClick={e => e.stopPropagation()}
      >
        <button className="settings-close-button" onClick={onClose} aria-label="Close">
          ✕
        </button>

        <div className="settings-layout">
          {/* Left sidebar navigation */}
          <nav className="settings-sidebar">
            <h2 className="settings-sidebar-title">
              <span className="settings-title-text">Settings</span>
              <span className="settings-title-icon">&#x2699;&#xFE0F;</span>
            </h2>
            <ul className="settings-nav-list">
              {PANELS.map(panel => {
                const status = getNavStatus(panel.id);
                return (
                  <li key={panel.id}>
                    <button
                      className={`settings-nav-item ${activePanel === panel.id ? 'active' : ''}`}
                      onClick={() => setActivePanel(panel.id)}
                    >
                      <span className="settings-nav-icon">{panel.icon}</span>
                      <span className="settings-nav-label">
                        {panel.label}
                        {status && ` [${status}]`}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
            <button className="settings-done-button" onClick={onClose}>Done</button>
          </nav>

          {/* Right panel content */}
          <div className="settings-panel-content">
            {activePanel === 'appearance' && <AppearancePanel />}
            {activePanel === 'spotify' && spotifyConfig && (
              <SpotifyPanel
                config={spotifyConfig}
                onConfigChange={setSpotifyConfig}
              />
            )}
            {activePanel === 'bulbs' && bulbConfig && (
              <BulbsPanel
                config={bulbConfig}
                onConfigChange={setBulbConfig}
              />
            )}
            {activePanel === 'downloads' && appSettings && (
              <DownloadsPanel
                settings={appSettings}
                onSettingsChange={setAppSettings}
              />
            )}
            {activePanel === 'user_content' && <UserContentPanel />}
          </div>
        </div>
      </div>
    </div>
  );
};

const AppearancePanel: FC = () => {
  const { theme, setTheme, resolvedTheme } = useTheme();

  const handleThemeChange = (newTheme: Theme) => {
    setTheme(newTheme);
  };

  return (
    <div className="settings-panel">
      <h3 className="settings-panel-title">Appearance</h3>
      <p className="settings-panel-description">
        Customize the look and feel of the application.
      </p>

      <div className="settings-section">
        <label className="settings-label">Theme</label>
        <div className="settings-theme-options">
          <button
            className={`settings-theme-button ${theme === 'light' ? 'active' : ''}`}
            onClick={() => handleThemeChange('light')}
          >
            <span className="settings-theme-icon">☀️</span>
            <span>Light</span>
          </button>
          <button
            className={`settings-theme-button ${theme === 'dark' ? 'active' : ''}`}
            onClick={() => handleThemeChange('dark')}
          >
            <span className="settings-theme-icon">🌙</span>
            <span>Dark</span>
          </button>
          <button
            className={`settings-theme-button ${theme === 'system' ? 'active' : ''}`}
            onClick={() => handleThemeChange('system')}
          >
            <span className="settings-theme-icon">💻</span>
            <span>System</span>
          </button>
        </div>
        {theme === 'system' && (
          <p className="settings-theme-note">
            Currently using: {resolvedTheme === 'dark' ? '🌙 Dark' : '☀️ Light'}
          </p>
        )}
      </div>
    </div>
  );
};

interface SpotifyPanelProps {
  config: SpotifyConfig;
  onConfigChange: (config: SpotifyConfig) => void;
}

const SpotifyPanel: FC<SpotifyPanelProps> = ({ config, onConfigChange }) => {
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const handleSave = async () => {
    setSaving(true);
    setMessage(null);
    try {
      await invoke('save_spotify_config', { config });
      // Update is_configured status
      const isConfigured = !!(config.client_id && config.client_secret);
      onConfigChange({ ...config, is_configured: isConfigured });
      setMessage({ type: 'success', text: 'Spotify settings saved successfully.' });
    } catch (e) {
      setMessage({ type: 'error', text: `Failed to save: ${e}` });
    } finally {
      setSaving(false);
    }
  };

  const handleFieldChange = (field: keyof SpotifyConfig, value: string) => {
    onConfigChange({ ...config, [field]: value });
  };

  return (
    <div className="settings-panel">
      <h3 className="settings-panel-title">Spotify</h3>
      <p className="settings-panel-description">
        Configure Spotify API credentials for music playback.
      </p>

      <div className="settings-section">
        <label className="settings-label">Status</label>
        <div className={`settings-status ${config.is_configured ? 'configured' : 'not-configured'}`}>
          <span className="settings-status-icon">{config.is_configured ? '✓' : '⚠️'}</span>
          <span>{config.is_configured ? 'Spotify is configured' : 'Not configured - music playback disabled'}</span>
        </div>
      </div>

      <div className="settings-section">
        <label className="settings-label">Username</label>
        <input
          type="text"
          className="settings-input"
          placeholder="Your Spotify username"
          value={config.username}
          onChange={e => handleFieldChange('username', e.target.value)}
        />
      </div>

      <div className="settings-section">
        <label className="settings-label">Client ID</label>
        <input
          type="text"
          className="settings-input"
          placeholder="From Spotify Developer Dashboard"
          value={config.client_id}
          onChange={e => handleFieldChange('client_id', e.target.value)}
        />
      </div>

      <div className="settings-section">
        <label className="settings-label">Client Secret</label>
        <input
          type="password"
          className="settings-input"
          placeholder="From Spotify Developer Dashboard"
          value={config.client_secret}
          onChange={e => handleFieldChange('client_secret', e.target.value)}
        />
      </div>

      <div className="settings-section">
        <label className="settings-label">Redirect URI</label>
        <input
          type="text"
          className="settings-input"
          placeholder="http://127.0.0.1:8888/callback"
          value={config.redirect_uri}
          onChange={e => handleFieldChange('redirect_uri', e.target.value)}
        />
      </div>

      <div className="settings-section">
        <label className="settings-label">Startup Behavior</label>
        <p className="settings-help-text">When Spotify is not playing on this PC:</p>
        <div className="settings-radio-group">
          <label className="settings-radio">
            <input
              type="radio"
              name="auto_start"
              value="ask"
              checked={config.auto_start === 'ask'}
              onChange={() => handleFieldChange('auto_start', 'ask')}
            />
            <span>Ask me what to do</span>
          </label>
          <label className="settings-radio">
            <input
              type="radio"
              name="auto_start"
              value="start_local"
              checked={config.auto_start === 'start_local'}
              onChange={() => handleFieldChange('auto_start', 'start_local')}
            />
            <span>Start Spotify on this PC</span>
          </label>
          <label className="settings-radio">
            <input
              type="radio"
              name="auto_start"
              value="use_remote"
              checked={config.auto_start === 'use_remote'}
              onChange={() => handleFieldChange('auto_start', 'use_remote')}
            />
            <span>Connect to Spotify on another device</span>
          </label>
          <label className="settings-radio">
            <input
              type="radio"
              name="auto_start"
              value="disabled"
              checked={config.auto_start === 'disabled'}
              onChange={() => handleFieldChange('auto_start', 'disabled')}
            />
            <span>Run without music</span>
          </label>
        </div>
      </div>

      <div className="settings-actions">
        <button className="settings-button" onClick={handleSave} disabled={saving}>
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </div>

      {message && (
        <div className={`settings-message ${message.type}`}>
          {message.text}
        </div>
      )}

      <div className="settings-help">
        <h4>Setup Instructions</h4>
        <ol>
          <li>Go to <a href="https://developer.spotify.com/dashboard" target="_blank" rel="noopener noreferrer">Spotify Developer Dashboard</a></li>
          <li>Create a new app</li>
          <li>Copy the Client ID and Client Secret here</li>
          <li>Add <code>http://127.0.0.1:8888/callback</code> to Redirect URIs in your app settings</li>
        </ol>
        <p className="settings-note">Note: Spotify Premium is required for playback control.</p>
      </div>
    </div>
  );
};

interface BulbsPanelProps {
  config: WizBulbConfig;
  onConfigChange: (config: WizBulbConfig) => void;
}

const BulbsPanel: FC<BulbsPanelProps> = ({ config, onConfigChange }) => {
  const [saving, setSaving] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [discoveryResults, setDiscoveryResults] = useState<string>('');
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const handleSave = async () => {
    setSaving(true);
    setMessage(null);
    try {
      await invoke('save_wizbulb_config', { config });
      // Update is_configured status
      const isConfigured = !!(config.backdrop_bulbs || config.overhead_bulbs || config.battlefield_bulbs);
      onConfigChange({ ...config, is_configured: isConfigured });
      setMessage({ type: 'success', text: 'WIZ bulb settings saved. Changes take effect on next environment activation.' });
    } catch (e) {
      setMessage({ type: 'error', text: `Failed to save: ${e}` });
    } finally {
      setSaving(false);
    }
  };

  const handleDiscover = async () => {
    setDiscovering(true);
    setDiscoveryResults('Discovering bulbs... (this may take a few seconds)');
    try {
      const bulbs = await invoke<string[]>('discover_bulbs');
      if (bulbs.length > 0) {
        setDiscoveryResults(
          `Found ${bulbs.length} bulb(s):\n\n${bulbs.map(ip => `  ${ip}`).join('\n')}\n\nCopy these IPs to the fields above.`
        );
      } else {
        setDiscoveryResults(
          'No bulbs found.\n\n' +
          'Make sure:\n' +
          '- Bulbs are powered on\n' +
          '- Bulbs are connected to same WiFi network\n' +
          '- Your firewall allows UDP broadcast'
        );
      }
    } catch (e) {
      setDiscoveryResults(`Discovery failed:\n${e}`);
    } finally {
      setDiscovering(false);
    }
  };

  const handleFieldChange = (field: keyof WizBulbConfig, value: string) => {
    onConfigChange({ ...config, [field]: value });
  };

  return (
    <div className="settings-panel">
      <h3 className="settings-panel-title">WIZ Bulbs</h3>
      <p className="settings-panel-description">
        Configure WIZ smart bulb IP addresses for lighting control.
      </p>

      <div className="settings-section">
        <label className="settings-label">Status</label>
        <div className={`settings-status ${config.is_configured ? 'configured' : 'not-configured'}`}>
          <span className="settings-status-icon">{config.is_configured ? '✓' : '⚠️'}</span>
          <span>{config.is_configured ? 'WIZ bulbs are configured' : 'No bulbs configured - lighting effects disabled'}</span>
        </div>
      </div>

      <div className="settings-section">
        <label className="settings-label">Backdrop Bulbs</label>
        <input
          type="text"
          className="settings-input"
          placeholder="192.168.1.100 192.168.1.101 192.168.1.102"
          value={config.backdrop_bulbs}
          onChange={e => handleFieldChange('backdrop_bulbs', e.target.value)}
        />
        <p className="settings-help-text">Ambient/background lighting (space-separated IPs)</p>
      </div>

      <div className="settings-section">
        <label className="settings-label">Overhead Bulbs</label>
        <input
          type="text"
          className="settings-input"
          placeholder="192.168.1.103 192.168.1.104"
          value={config.overhead_bulbs}
          onChange={e => handleFieldChange('overhead_bulbs', e.target.value)}
        />
        <p className="settings-help-text">Main room lighting</p>
      </div>

      <div className="settings-section">
        <label className="settings-label">Battlefield Bulbs</label>
        <input
          type="text"
          className="settings-input"
          placeholder="192.168.1.105"
          value={config.battlefield_bulbs}
          onChange={e => handleFieldChange('battlefield_bulbs', e.target.value)}
        />
        <p className="settings-help-text">Dramatic/combat lighting</p>
      </div>

      <div className="settings-actions">
        <button className="settings-button" onClick={handleDiscover} disabled={discovering}>
          {discovering ? 'Discovering...' : 'Discover Bulbs on Network'}
        </button>
        <button className="settings-button" onClick={handleSave} disabled={saving}>
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </div>

      {discoveryResults && (
        <div className="settings-discovery-results">
          <pre>{discoveryResults}</pre>
        </div>
      )}

      {message && (
        <div className={`settings-message ${message.type}`}>
          {message.text}
        </div>
      )}
    </div>
  );
};

interface DownloadsPanelProps {
  settings: AppSettings;
  onSettingsChange: (settings: AppSettings) => void;
}

const DownloadsPanel: FC<DownloadsPanelProps> = ({ settings, onSettingsChange }) => {
  const [saving, setSaving] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [reloading, setReloading] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const handleSettingChange = async (key: keyof AppSettings, value: boolean | string) => {
    setSaving(true);
    setMessage(null);
    const newSettings = { ...settings, [key]: value };
    try {
      await invoke('save_app_settings', { settings: newSettings });
      onSettingsChange(newSettings);
      setMessage({ type: 'success', text: 'Settings saved.' });
    } catch (e) {
      setMessage({ type: 'error', text: `Failed to save: ${e}` });
    } finally {
      setSaving(false);
    }
  };

  const handleClearSoundCache = async () => {
    setClearing(true);
    setMessage(null);
    try {
      const count = await invoke<number>('clear_sound_cache');
      setMessage({ type: 'success', text: `Cleared ${count} file${count !== 1 ? 's' : ''}. Reloading...` });
      // Reload the app so frontend re-fetches all configs
      setTimeout(() => window.location.reload(), 800);
    } catch (e) {
      setMessage({ type: 'error', text: `Failed to clear cache: ${e}` });
      setClearing(false);
    }
  };

  const handleReloadConfigs = async () => {
    setReloading(true);
    setMessage(null);
    try {
      const count = await invoke<number>('reload_configs');
      setMessage({ type: 'success', text: `Reloaded ${count} configs. Reloading...` });
      // Reload the app so frontend re-fetches all configs
      setTimeout(() => window.location.reload(), 800);
    } catch (e) {
      setMessage({ type: 'error', text: `Failed to reload configs: ${e}` });
      setReloading(false);
    }
  };

  return (
    <div className="settings-panel">
      <h3 className="settings-panel-title">Downloads</h3>
      <p className="settings-panel-description">
        Atmosphere sounds are bundled with the app. On-demand downloads are only
        needed if you add custom environments with freesound URLs that aren't bundled.
      </p>

      <div className="settings-section">
        <label className="settings-label">On-Demand Downloads</label>
        <div className="settings-checkbox-group">
          <label className="settings-checkbox">
            <input
              type="checkbox"
              checked={settings.downloads_enabled}
              onChange={e => handleSettingChange('downloads_enabled', e.target.checked)}
              disabled={saving}
            />
            <span>Enable on-demand downloads from freesound.org</span>
          </label>
        </div>
        <p className="settings-help-text">
          When disabled, only bundled sounds play. Enable this if you add custom
          environments that reference freesound URLs not included in the bundle.
        </p>
      </div>

      <div className="settings-section">
        <label className="settings-label">SSL Certificate Verification</label>
        <div className="settings-checkbox-group">
          <label className="settings-checkbox">
            <input
              type="checkbox"
              checked={settings.ignore_ssl_errors}
              onChange={e => handleSettingChange('ignore_ssl_errors', e.target.checked)}
              disabled={saving}
            />
            <span>Ignore SSL certificate errors</span>
          </label>
        </div>

        <div className="settings-warning">
          <p>
            Enable this if you're behind a corporate VPN or proxy that performs
            SSL inspection (MITM). This allows freesound.org downloads to work
            when certificate verification fails.
          </p>
          <p className="settings-warning-caution">
            <strong>Warning:</strong> Only enable if you trust your network.
            Disabling SSL verification can expose you to man-in-the-middle attacks
            on untrusted networks.
          </p>
        </div>
      </div>

      <div className="settings-section">
        <label className="settings-label">Cache Management</label>
        <p className="settings-help-text">
          Clear cached data to fix playback issues or free up storage.
        </p>
        <div className="settings-actions">
          <button
            className="settings-button"
            onClick={handleClearSoundCache}
            disabled={clearing}
          >
            {clearing ? 'Clearing...' : 'Clear Sound Cache'}
          </button>
          <button
            className="settings-button"
            onClick={handleReloadConfigs}
            disabled={reloading}
          >
            {reloading ? 'Reloading...' : 'Reload Configs'}
          </button>
        </div>
        <p className="settings-help-text" style={{ marginTop: '0.5rem' }}>
          <strong>Clear Sound Cache</strong> removes all downloaded freesound files.
          They will be re-downloaded when needed.
        </p>
        <p className="settings-help-text">
          <strong>Reload Configs</strong> re-reads all environment YAML files
          and regenerates sound categories.
        </p>
      </div>

      {message && (
        <div className={`settings-message ${message.type}`}>
          {message.text}
        </div>
      )}
    </div>
  );
};

const UserContentPanel: FC = () => {
  const [dirPath, setDirPath] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);

  useEffect(() => {
    invoke<string | null>('get_user_content_dir').then(setDirPath).catch(() => {});
  }, []);

  const handleOpenFolder = async () => {
    if (!dirPath) return;
    setOpening(true);
    try {
      await open(dirPath);
    } catch (e) {
      console.error('Failed to open folder:', e);
    } finally {
      setOpening(false);
    }
  };

  return (
    <div className="settings-panel">
      <h3 className="settings-panel-title">User Content</h3>
      <p className="settings-panel-description">
        Add custom environments, sound collections, and audio files.
        Content placed here is loaded alongside built-in configs.
      </p>

      <div className="settings-section">
        <label className="settings-label">Directory</label>
        {dirPath ? (
          <>
            <code className="settings-path">{dirPath}</code>
            <div className="settings-actions" style={{ marginTop: '0.5rem' }}>
              <button
                className="settings-button"
                onClick={handleOpenFolder}
                disabled={opening}
              >
                {opening ? 'Opening...' : 'Open Folder'}
              </button>
            </div>
          </>
        ) : (
          <p className="settings-help-text">User content directory not available.</p>
        )}
      </div>

      <div className="settings-section">
        <label className="settings-label">Directory Structure</label>
        <pre className="settings-discovery-results" style={{ margin: 0 }}>
{`env_conf/    \u2014 Environment YAML configs
sound_conf/  \u2014 Sound collection YAML configs
sounds/      \u2014 Audio files (.wav, .mp3, .ogg)`}</pre>
      </div>

      <div className="settings-help">
        <h4>How to Add Content</h4>
        <ol>
          <li>Open the folder above</li>
          <li>Place YAML configs in <code>env_conf/</code> (same schema as built-in environments)</li>
          <li>Place audio files in <code>sounds/</code></li>
          <li>Go to Settings &gt; Downloads &gt; <strong>Reload Configs</strong> to pick up changes</li>
        </ol>
        <p className="settings-note">
          Configs with the same filename as built-in ones will override them.
          Use any category name and it will appear in the sidebar automatically.
        </p>
      </div>
    </div>
  );
};
