//! Spotify engine for music playback control.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{Error, Result};

/// Spotify API base URL.
const SPOTIFY_API_BASE: &str = "https://api.spotify.com/v1";

/// Spotify accounts URL for token requests.
const SPOTIFY_ACCOUNTS_URL: &str = "https://accounts.spotify.com/api/token";

/// Spotify authorize URL.
const SPOTIFY_AUTH_URL: &str = "https://accounts.spotify.com/authorize";

/// Spotify engine for controlling playback.
pub struct SpotifyEngine {
    client: Client,
    token: Arc<RwLock<Option<AccessToken>>>,
    config: SpotifyCredentials,
    cache_path: PathBuf,
}

/// Spotify API credentials.
#[derive(Debug, Clone)]
pub struct SpotifyCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub username: String,
}

impl SpotifyCredentials {
    /// Loads credentials from a .spotify.ini file.
    pub fn from_config_file(path: &str) -> Result<Self> {
        let config = ini::Ini::load_from_file(path)
            .map_err(|e| Error::ConfigLoad(path.to_string(), e.to_string()))?;

        let section = config.section(Some("DEFAULT")).ok_or_else(|| {
            Error::SpotifyNotConfigured
        })?;

        Ok(Self {
            client_id: section
                .get("client_id")
                .ok_or(Error::SpotifyNotConfigured)?
                .to_string(),
            client_secret: section
                .get("client_secret")
                .ok_or(Error::SpotifyNotConfigured)?
                .to_string(),
            redirect_uri: section
                .get("redirectURI")
                .unwrap_or("http://127.0.0.1:8888/callback")
                .to_string(),
            username: section
                .get("username")
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Returns true if credentials appear to be configured.
    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty() && !self.client_secret.is_empty()
    }
}

/// OAuth access token with expiry tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessToken {
    access_token: String,
    token_type: String,
    expires_at: u64,
    refresh_token: Option<String>,
    scope: Option<String>,
}

impl AccessToken {
    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now >= self.expires_at
    }
}

/// Spotify device information.
#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyDevice {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    #[serde(rename = "type")]
    pub device_type: String,
    pub is_private_session: bool,
    pub is_restricted: bool,
}

impl SpotifyEngine {
    /// Creates a new Spotify engine with the given credentials.
    pub fn new(config: SpotifyCredentials, cache_path: PathBuf) -> Self {
        Self {
            client: Client::new(),
            token: Arc::new(RwLock::new(None)),
            config,
            cache_path,
        }
    }

    /// Creates a Spotify engine from config files.
    pub fn from_config_files(spotify_ini: &str, cache_path: &str) -> Result<Self> {
        let config = SpotifyCredentials::from_config_file(spotify_ini)?;
        Ok(Self::new(config, PathBuf::from(cache_path)))
    }

    /// Authenticates with Spotify, using cached token if available.
    pub async fn authenticate(&self) -> Result<()> {
        // Try loading cached token
        if let Ok(token) = self.load_cached_token() {
            if !token.is_expired() {
                let mut guard = self.token.write().await;
                *guard = Some(token);
                return Ok(());
            }

            // Try refreshing
            if let Some(refresh_token) = &token.refresh_token {
                if let Ok(new_token) = self.refresh_token(refresh_token).await {
                    self.save_cached_token(&new_token)?;
                    let mut guard = self.token.write().await;
                    *guard = Some(new_token);
                    return Ok(());
                }
            }
        }

        // Need full OAuth flow
        let token = self.oauth_flow().await?;
        self.save_cached_token(&token)?;
        let mut guard = self.token.write().await;
        *guard = Some(token);
        Ok(())
    }

    /// Performs the OAuth authorization flow.
    async fn oauth_flow(&self) -> Result<AccessToken> {
        let scopes = "user-modify-playback-state user-read-playback-state user-read-currently-playing";

        // Generate state for CSRF protection (ASCII alphanumeric only —
        // Spotify rejects non-ASCII state parameters with "illegal state parameter").
        // Block scope ensures ThreadRng (which is !Send) is dropped before any .await.
        let state: String = {
            let mut rng = rand::thread_rng();
            (0..32)
                .map(|_| {
                    let idx = rng.gen_range(0..36u8);
                    if idx < 10 { (b'0' + idx) as char } else { (b'a' + idx - 10) as char }
                })
                .collect()
        };

        // Build authorization URL
        let auth_url = format!(
            "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}",
            SPOTIFY_AUTH_URL,
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(&self.config.redirect_uri),
            urlencoding::encode(scopes),
            urlencoding::encode(&state)
        );

        tracing::info!("Opening browser for Spotify authorization");
        tracing::info!("Auth URL: {}", auth_url);

        // Open browser
        let _ = open::that(&auth_url);

        // Start local server to receive callback
        let code = self.wait_for_callback(&state)?;

        // Exchange code for token
        self.exchange_code(&code).await
    }

    /// Waits for the OAuth callback on the redirect URI.
    fn wait_for_callback(&self, expected_state: &str) -> Result<String> {
        // Parse port from redirect URI
        let port: u16 = self
            .config
            .redirect_uri
            .split(':')
            .last()
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(8888);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .map_err(|e| Error::SpotifyAuth(format!("Failed to bind to port {}: {}", port, e)))?;

        listener
            .set_nonblocking(false)
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;

        tracing::info!("Waiting for Spotify callback on port {}...", port);

        // Accept one connection
        let (mut stream, _) = listener
            .accept()
            .map_err(|e| Error::SpotifyAuth(format!("Failed to accept connection: {}", e)))?;

        // Read the request
        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;

        // Parse the callback URL
        let url_part = request_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| Error::SpotifyAuth("Invalid callback request".to_string()))?;

        // Extract query parameters
        let query = url_part
            .split('?')
            .nth(1)
            .ok_or_else(|| Error::SpotifyAuth("No query parameters in callback".to_string()))?;

        let params: HashMap<&str, &str> = query
            .split('&')
            .filter_map(|p| {
                let mut parts = p.split('=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        // Verify state
        let received_state = params
            .get("state")
            .ok_or_else(|| Error::SpotifyAuth("Missing state parameter".to_string()))?;

        if *received_state != expected_state {
            return Err(Error::SpotifyAuth("State mismatch - possible CSRF attack".to_string()));
        }

        // Check for error
        if let Some(error) = params.get("error") {
            return Err(Error::SpotifyAuth(format!("Authorization error: {}", error)));
        }

        // Get the code
        let code = params
            .get("code")
            .ok_or_else(|| Error::SpotifyAuth("Missing code parameter".to_string()))?
            .to_string();

        // Send response to browser
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
            <html><body><h1>Authorization successful!</h1>\
            <p>You can close this window and return to Immerse Yourself.</p></body></html>";

        stream
            .write_all(response.as_bytes())
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;

        Ok(code)
    }

    /// Exchanges an authorization code for an access token.
    async fn exchange_code(&self, code: &str) -> Result<AccessToken> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.config.redirect_uri),
        ];

        let response = self
            .client
            .post(SPOTIFY_ACCOUNTS_URL)
            .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
            .form(&params)
            .send()
            .await
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::SpotifyAuth(format!("Token exchange failed: {}", error)));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            token_type: String,
            expires_in: u64,
            refresh_token: Option<String>,
            scope: Option<String>,
        }

        let token_resp: TokenResponse = response
            .json()
            .await
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;

        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + token_resp.expires_in;

        Ok(AccessToken {
            access_token: token_resp.access_token,
            token_type: token_resp.token_type,
            expires_at,
            refresh_token: token_resp.refresh_token,
            scope: token_resp.scope,
        })
    }

    /// Refreshes an expired access token.
    async fn refresh_token(&self, refresh_token: &str) -> Result<AccessToken> {
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        let response = self
            .client
            .post(SPOTIFY_ACCOUNTS_URL)
            .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
            .form(&params)
            .send()
            .await
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::SpotifyAuth(format!("Token refresh failed: {}", error)));
        }

        #[derive(Deserialize)]
        struct RefreshResponse {
            access_token: String,
            token_type: String,
            expires_in: u64,
            scope: Option<String>,
        }

        let resp: RefreshResponse = response
            .json()
            .await
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;

        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + resp.expires_in;

        Ok(AccessToken {
            access_token: resp.access_token,
            token_type: resp.token_type,
            expires_at,
            refresh_token: Some(refresh_token.to_string()),
            scope: resp.scope,
        })
    }

    /// Loads a cached token from disk.
    fn load_cached_token(&self) -> Result<AccessToken> {
        let content = fs::read_to_string(&self.cache_path)
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|e| Error::SpotifyAuth(e.to_string()))
    }

    /// Saves a token to the cache file.
    fn save_cached_token(&self, token: &AccessToken) -> Result<()> {
        let content = serde_json::to_string_pretty(token)
            .map_err(|e| Error::SpotifyAuth(e.to_string()))?;
        fs::write(&self.cache_path, content)
            .map_err(|e| Error::SpotifyAuth(e.to_string()))
    }

    /// Gets the current access token, refreshing if needed.
    async fn get_token(&self) -> Result<String> {
        let guard = self.token.read().await;
        let token = guard.as_ref().ok_or(Error::SpotifyNotConfigured)?;

        if token.is_expired() {
            drop(guard);
            self.authenticate().await?;
            let guard = self.token.read().await;
            Ok(guard.as_ref().unwrap().access_token.clone())
        } else {
            Ok(token.access_token.clone())
        }
    }

    /// Plays a Spotify context (playlist, album, etc.).
    /// Automatically handles "no active device" by transferring to first available device.
    pub async fn play_context(&self, context_uri: &str) -> Result<()> {
        // First attempt
        match self.play_context_internal(context_uri).await {
            Ok(()) => return Ok(()),
            Err(Error::SpotifyNoDevice) => {
                tracing::info!("No active Spotify device, attempting to activate one...");
            }
            Err(e) => return Err(e),
        }

        // No active device - try to find and activate one
        let devices = self.get_devices().await?;
        if devices.is_empty() {
            tracing::warn!("No Spotify devices available");
            return Err(Error::SpotifyNoDevice);
        }

        // Find the first available device (prefer local computer)
        let device = devices
            .iter()
            .find(|d| d.device_type == "Computer")
            .or(devices.first())
            .ok_or(Error::SpotifyNoDevice)?;

        tracing::info!("Transferring playback to device: {} ({})", device.name, device.id);

        // Transfer playback to this device
        self.transfer_playback(&device.id, false).await?;

        // Wait a moment for device to be ready
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Retry playback
        self.play_context_internal(context_uri).await
    }

    /// Internal method to play without device retry logic.
    async fn play_context_internal(&self, context_uri: &str) -> Result<()> {
        let token = self.get_token().await?;

        // Enable shuffle for variability (ignore errors - may fail if no device)
        let _ = self.set_shuffle(true).await;

        let body = serde_json::json!({
            "context_uri": context_uri
        });

        let response = self
            .client
            .put(format!("{}/me/player/play", SPOTIFY_API_BASE))
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::SpotifyApi(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();

            if status.as_u16() == 404 {
                return Err(Error::SpotifyNoDevice);
            }

            return Err(Error::SpotifyApi(format!(
                "Play failed ({}): {}",
                status, error
            )));
        }

        // Skip to next track so each session starts differently (ignore errors)
        let _ = self.skip_to_next().await;

        Ok(())
    }

    /// Sets shuffle mode.
    async fn set_shuffle(&self, state: bool) -> Result<()> {
        let token = self.get_token().await?;

        let response = self
            .client
            .put(format!("{}/me/player/shuffle?state={}", SPOTIFY_API_BASE, state))
            .bearer_auth(&token)
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| Error::SpotifyApi(e.to_string()))?;

        if !response.status().is_success() && response.status().as_u16() != 404 {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::SpotifyApi(format!("Shuffle failed: {}", error)));
        }

        Ok(())
    }

    /// Skips to the next track.
    async fn skip_to_next(&self) -> Result<()> {
        let token = self.get_token().await?;

        let response = self
            .client
            .post(format!("{}/me/player/next", SPOTIFY_API_BASE))
            .bearer_auth(&token)
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| Error::SpotifyApi(e.to_string()))?;

        if !response.status().is_success() && response.status().as_u16() != 404 {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::SpotifyApi(format!("Skip failed: {}", error)));
        }

        Ok(())
    }

    /// Pauses playback.
    pub async fn pause(&self) -> Result<()> {
        let token = self.get_token().await?;

        let response = self
            .client
            .put(format!("{}/me/player/pause", SPOTIFY_API_BASE))
            .bearer_auth(&token)
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| Error::SpotifyApi(e.to_string()))?;

        // 404 means no active device, which is fine for pause
        if !response.status().is_success() && response.status().as_u16() != 404 {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::SpotifyApi(format!("Pause failed: {}", error)));
        }

        Ok(())
    }

    /// Gets available playback devices.
    pub async fn get_devices(&self) -> Result<Vec<SpotifyDevice>> {
        let token = self.get_token().await?;

        let response = self
            .client
            .get(format!("{}/me/player/devices", SPOTIFY_API_BASE))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| Error::SpotifyApi(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::SpotifyApi(format!("Get devices failed: {}", error)));
        }

        #[derive(Deserialize)]
        struct DevicesResponse {
            devices: Vec<SpotifyDevice>,
        }

        let resp: DevicesResponse = response
            .json()
            .await
            .map_err(|e| Error::SpotifyApi(e.to_string()))?;

        Ok(resp.devices)
    }

    /// Transfers playback to a specific device.
    pub async fn transfer_playback(&self, device_id: &str, play: bool) -> Result<()> {
        let token = self.get_token().await?;

        let body = serde_json::json!({
            "device_ids": [device_id],
            "play": play
        });

        let response = self
            .client
            .put(format!("{}/me/player", SPOTIFY_API_BASE))
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::SpotifyApi(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::SpotifyApi(format!("Transfer failed: {}", error)));
        }

        Ok(())
    }

    /// Returns whether Spotify is configured.
    pub fn is_configured(&self) -> bool {
        self.config.is_configured()
    }

    /// Returns whether Spotify has a cached, non-expired access token on disk.
    /// This indicates auth has completed successfully at least once recently.
    /// Sync — safe to call from any context without holding async locks.
    pub fn has_cached_token(&self) -> bool {
        self.load_cached_token().map_or(false, |t| !t.is_expired())
    }
}

/// Checks if Spotify is running on the system.
pub fn is_spotify_running() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("pgrep")
            .args(["-x", "spotify"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("pgrep")
            .args(["-x", "Spotify"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Spotify.exe"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("Spotify.exe"))
            .unwrap_or(false)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// Attempts to start Spotify.
pub fn start_spotify() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("spotify")
            .spawn()
            .map_err(|e| Error::SpotifyApi(format!("Failed to start Spotify: {}", e)))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Spotify"])
            .spawn()
            .map_err(|e| Error::SpotifyApi(format!("Failed to start Spotify: {}", e)))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("spotify")
            .spawn()
            .map_err(|e| Error::SpotifyApi(format!("Failed to start Spotify: {}", e)))?;
    }

    Ok(())
}

/// Checks if Spotify is in PATH.
pub fn is_spotify_in_path() -> bool {
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        std::process::Command::new("which")
            .arg("spotify")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_token_expiry() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired = AccessToken {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: now - 100,
            refresh_token: None,
            scope: None,
        };
        assert!(expired.is_expired());

        let valid = AccessToken {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_at: now + 3600,
            refresh_token: None,
            scope: None,
        };
        assert!(!valid.is_expired());
    }

    #[test]
    fn test_is_spotify_in_path() {
        // Just verify it doesn't panic
        let _ = is_spotify_in_path();
    }
}
