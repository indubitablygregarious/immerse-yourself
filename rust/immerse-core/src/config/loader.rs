//! YAML configuration loader with caching and discovery.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::config::types::EnvironmentConfig;
use crate::config::validator::ConfigValidator;
use crate::error::{Error, Result};

/// Configuration loader with caching.
/// Supports scanning multiple directories (e.g., built-in + user content).
/// When names collide, later directories override earlier ones.
pub struct ConfigLoader {
    config_dirs: Vec<PathBuf>,
    cache: Arc<RwLock<HashMap<String, EnvironmentConfig>>>,
    validator: ConfigValidator,
}

impl ConfigLoader {
    /// Creates a new config loader for a single directory.
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Self {
        Self {
            config_dirs: vec![config_dir.as_ref().to_path_buf()],
            cache: Arc::new(RwLock::new(HashMap::new())),
            validator: ConfigValidator::new(),
        }
    }

    /// Creates a new config loader that scans multiple directories.
    /// Directories are scanned in order; configs from later directories
    /// override earlier ones when names collide.
    pub fn new_with_dirs(config_dirs: Vec<PathBuf>) -> Self {
        Self {
            config_dirs,
            cache: Arc::new(RwLock::new(HashMap::new())),
            validator: ConfigValidator::new(),
        }
    }

    /// Loads a single config by filename (searches all directories).
    pub fn load(&self, filename: &str) -> Result<EnvironmentConfig> {
        // Check cache first
        {
            let cache = self.cache.read().map_err(|_| Error::CacheLock)?;
            if let Some(config) = cache.get(filename) {
                return Ok(config.clone());
            }
        }

        // Search directories in reverse order (later dirs take priority)
        let mut path = None;
        for dir in self.config_dirs.iter().rev() {
            let candidate = dir.join(filename);
            if candidate.exists() {
                path = Some(candidate);
                break;
            }
        }

        let path = path.ok_or_else(|| {
            Error::ConfigLoad(
                filename.to_string(),
                "File not found in any config directory".to_string(),
            )
        })?;

        let config = Self::load_from_path(&path)?;

        // Validate
        self.validator.validate(&config)?;

        // Cache it
        {
            let mut cache = self.cache.write().map_err(|_| Error::CacheLock)?;
            cache.insert(filename.to_string(), config.clone());
        }

        Ok(config)
    }

    /// Loads config from a specific path, setting the source_path field.
    fn load_from_path(path: &Path) -> Result<EnvironmentConfig> {
        let content = fs::read_to_string(path)
            .map_err(|e| Error::ConfigLoad(path.display().to_string(), e.to_string()))?;

        let mut config: EnvironmentConfig = serde_yaml::from_str(&content)
            .map_err(|e| Error::ConfigParse(path.display().to_string(), e.to_string()))?;

        config.source_path = Some(path.to_path_buf());
        Ok(config)
    }

    /// Discovers all config files across all directories.
    /// Returns (filename, full_path) pairs. Later directories override earlier.
    fn discover_all_with_paths(&self) -> Result<Vec<(String, PathBuf)>> {
        let mut by_name: HashMap<String, PathBuf> = HashMap::new();

        for dir in &self.config_dirs {
            if !dir.exists() {
                continue;
            }
            let entries = fs::read_dir(dir)
                .map_err(|e| Error::ConfigLoad(dir.display().to_string(), e.to_string()))?;

            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
                    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                        by_name.insert(filename.to_string(), path);
                    }
                }
            }
        }

        let mut configs: Vec<(String, PathBuf)> = by_name.into_iter().collect();
        configs.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(configs)
    }

    /// Discovers all config filenames across all directories.
    pub fn discover_all(&self) -> Result<Vec<String>> {
        Ok(self
            .discover_all_with_paths()?
            .into_iter()
            .map(|(name, _)| name)
            .collect())
    }

    /// Loads all configs from all directories, returning them grouped by category.
    /// Configs from later directories override earlier ones when filenames match.
    pub fn load_all(&self) -> Result<HashMap<String, Vec<EnvironmentConfig>>> {
        let file_entries = self.discover_all_with_paths()?;
        let mut by_category: HashMap<String, Vec<EnvironmentConfig>> = HashMap::new();

        for (filename, path) in file_entries {
            match Self::load_from_path(&path) {
                Ok(config) => {
                    if let Err(e) = self.validator.validate(&config) {
                        tracing::warn!("Config {} failed validation: {}", filename, e);
                        continue;
                    }
                    // Cache it
                    if let Ok(mut cache) = self.cache.write() {
                        cache.insert(filename, config.clone());
                    }
                    by_category
                        .entry(config.category.clone())
                        .or_default()
                        .push(config);
                }
                Err(e) => {
                    tracing::warn!("Failed to load config {}: {}", filename, e);
                }
            }
        }

        // Sort configs within each category by name
        for configs in by_category.values_mut() {
            configs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }

        Ok(by_category)
    }

    /// Clears the config cache.
    pub fn clear_cache(&self) -> Result<()> {
        let mut cache = self.cache.write().map_err(|_| Error::CacheLock)?;
        cache.clear();
        Ok(())
    }

    /// Returns the primary (first) config directory path.
    pub fn config_dir(&self) -> &Path {
        self.config_dirs.first().map(|p| p.as_path()).unwrap_or(Path::new("."))
    }

    /// Returns all config directories.
    pub fn config_dirs(&self) -> &[PathBuf] {
        &self.config_dirs
    }
}

/// Canonical time periods in order (matches Python TIME_PERIODS).
pub const TIME_PERIODS: &[&str] = &["morning", "daytime", "afternoon", "evening"];

/// Checks if a config has inline time_variants.
/// Accepts (config_dir, base_name) for backward compatibility.
pub fn has_time_variants(config_dir: &Path, base_name: &str) -> bool {
    has_time_variants_at_path(&config_dir.join(base_name))
}

/// Checks if a config file has inline time_variants by full path.
pub fn has_time_variants_at_path(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    match fs::read_to_string(path) {
        Ok(content) => content.contains("time_variants:"),
        Err(_) => false,
    }
}

/// Resolves the time variant filename for a base config.
/// Note: This is kept for backward compatibility but inline variants are preferred.
pub fn resolve_time_variant(base_name: &str, time: &str) -> String {
    let base_stem = base_name.trim_end_matches(".yaml").trim_end_matches(".yml");
    format!("{}_{}.yaml", base_stem, time)
}

/// Gets available time variants for a config by reading its time_variants keys.
/// Accepts (config_dir, base_name) for backward compatibility.
/// Returns times in canonical order: morning, daytime, afternoon, evening.
pub fn get_available_times(config_dir: &Path, base_name: &str) -> Vec<String> {
    get_available_times_at_path(&config_dir.join(base_name))
}

/// Gets available time variants by full path.
pub fn get_available_times_at_path(path: &Path) -> Vec<String> {
    if !path.exists() {
        return Vec::new();
    }

    match fs::read_to_string(path) {
        Ok(content) => {
            if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(time_variants) = value.get("time_variants") {
                    if let Some(mapping) = time_variants.as_mapping() {
                        return TIME_PERIODS
                            .iter()
                            .filter(|t| {
                                mapping
                                    .get(serde_yaml::Value::String(t.to_string()))
                                    .is_some()
                            })
                            .map(|t| t.to_string())
                            .collect();
                    }
                }
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

/// Gets the time variant configuration from a config's time_variants section.
/// Accepts (config_dir, base_name) for backward compatibility.
pub fn get_time_variant_engines(
    config_dir: &Path,
    base_name: &str,
    time: &str,
) -> Option<serde_yaml::Value> {
    get_time_variant_engines_at_path(&config_dir.join(base_name), time)
}

/// Gets the time variant engines from a config file by full path.
/// If time is "daytime", returns None (use base config).
pub fn get_time_variant_engines_at_path(
    path: &Path,
    time: &str,
) -> Option<serde_yaml::Value> {
    if time == "daytime" {
        return None;
    }

    if !path.exists() {
        return None;
    }

    match fs::read_to_string(path) {
        Ok(content) => {
            if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(time_variants) = value.get("time_variants") {
                    if let Some(variant) = time_variants.get(time) {
                        return variant.get("engines").cloned();
                    }
                }
            }
            None
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_config(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        let mut file = fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_content = r#"
name: "Test Environment"
category: "test"
description: "A test config"
engines:
  sound:
    enabled: true
    file: "test.wav"
"#;
        create_test_config(temp_dir.path(), "test.yaml", config_content);

        let loader = ConfigLoader::new(temp_dir.path());
        let config = loader.load("test.yaml").unwrap();

        assert_eq!(config.name, "Test Environment");
        assert_eq!(config.category, "test");
        assert!(config.source_path.is_some());
    }

    #[test]
    fn test_discover_all() {
        let temp_dir = TempDir::new().unwrap();

        create_test_config(
            temp_dir.path(),
            "first.yaml",
            "name: First\ncategory: test\nengines: {}",
        );
        create_test_config(
            temp_dir.path(),
            "second.yaml",
            "name: Second\ncategory: test\nengines: {}",
        );

        let loader = ConfigLoader::new(temp_dir.path());
        let configs = loader.discover_all().unwrap();

        assert_eq!(configs.len(), 2);
        assert!(configs.contains(&"first.yaml".to_string()));
        assert!(configs.contains(&"second.yaml".to_string()));
    }

    #[test]
    fn test_multi_dir_override() {
        let builtin_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();

        create_test_config(
            builtin_dir.path(),
            "test.yaml",
            "name: Built-in\ncategory: test\nengines: {}",
        );
        create_test_config(
            user_dir.path(),
            "test.yaml",
            "name: User Override\ncategory: test\nengines: {}",
        );

        let loader = ConfigLoader::new_with_dirs(vec![
            builtin_dir.path().to_path_buf(),
            user_dir.path().to_path_buf(),
        ]);

        let configs = loader.load_all().unwrap();
        let test_configs = configs.get("test").unwrap();
        // User dir overrides built-in (same filename)
        assert_eq!(test_configs.len(), 1);
        assert_eq!(test_configs[0].name, "User Override");
    }

    #[test]
    fn test_multi_dir_merge() {
        let builtin_dir = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();

        create_test_config(
            builtin_dir.path(),
            "builtin.yaml",
            "name: Built-in\ncategory: test\nengines: {}",
        );
        create_test_config(
            user_dir.path(),
            "custom.yaml",
            "name: Custom\ncategory: test\nengines: {}",
        );

        let loader = ConfigLoader::new_with_dirs(vec![
            builtin_dir.path().to_path_buf(),
            user_dir.path().to_path_buf(),
        ]);

        let configs = loader.load_all().unwrap();
        let test_configs = configs.get("test").unwrap();
        assert_eq!(test_configs.len(), 2);
    }

    #[test]
    fn test_missing_user_dir() {
        let builtin_dir = TempDir::new().unwrap();
        let fake_user_dir = PathBuf::from("/tmp/nonexistent_user_content_dir_test");

        create_test_config(
            builtin_dir.path(),
            "test.yaml",
            "name: Test\ncategory: test\nengines: {}",
        );

        let loader = ConfigLoader::new_with_dirs(vec![
            builtin_dir.path().to_path_buf(),
            fake_user_dir,
        ]);

        let configs = loader.load_all().unwrap();
        assert_eq!(configs.get("test").unwrap().len(), 1);
    }

    #[test]
    fn test_time_variant_resolution() {
        assert_eq!(
            resolve_time_variant("forest.yaml", "morning"),
            "forest_morning.yaml"
        );
        assert_eq!(
            resolve_time_variant("tavern", "night"),
            "tavern_night.yaml"
        );
    }
}
