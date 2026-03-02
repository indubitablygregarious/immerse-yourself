//! Sound manifest utilities for freesound.org URL resolution.
//!
//! Provides helpers for parsing freesound URLs, loading manifest files that map
//! URLs to local file paths, and finding cached sound files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use regex::Regex;

/// Parses a freesound.org URL to extract creator and sound ID.
pub fn parse_freesound_url(url: &str) -> Option<(String, String)> {
    let re = Regex::new(r"freesound\.org/people/([^/]+)/sounds/(\d+)").ok()?;
    let caps = re.captures(url)?;
    Some((caps[1].to_string(), caps[2].to_string()))
}

/// Finds a downloaded file matching creator and sound ID.
pub fn find_downloaded_file(cache_dir: &Path, creator: &str, sound_id: &str) -> Option<PathBuf> {
    let prefix = format!("{}_{}_", creator, sound_id);
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Loads a sound manifest file and returns a map of freesound URLs to absolute file paths.
pub fn load_sound_manifest(base_dir: &Path, manifest_path: &Path) -> HashMap<String, PathBuf> {
    let mut result = HashMap::new();
    let contents = match std::fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(_) => return result,
    };
    let raw: HashMap<String, String> = match serde_json::from_str(&contents) {
        Ok(m) => m,
        Err(_) => return result,
    };
    for (url, rel_path) in raw {
        let abs_path = base_dir.join(&rel_path);
        if abs_path.exists() {
            result.insert(url, abs_path);
        }
    }
    result
}

/// Checks if a URL is a valid freesound.org URL.
pub fn is_freesound_url(url: &str) -> bool {
    parse_freesound_url(url).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_freesound_url() {
        let url = "https://freesound.org/people/klankbeeld/sounds/625333/";
        let parsed = parse_freesound_url(url);
        assert_eq!(parsed, Some(("klankbeeld".to_string(), "625333".to_string())));
    }

    #[test]
    fn test_is_freesound_url() {
        assert!(is_freesound_url("https://freesound.org/people/user/sounds/12345/"));
        assert!(!is_freesound_url("https://example.com/sound.mp3"));
    }
}
