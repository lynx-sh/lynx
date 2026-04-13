pub mod defaults;
pub mod migrate;
pub mod schema;
pub mod snapshot;
pub mod validate;

use lynx_core::error::{LynxError, Result};
use schema::LynxConfig;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

/// Process-level config cache. Populated on first `load()` call; cleared by `invalidate_cache()`.
/// `load_from()` bypasses this cache and always reads from disk (used in tests and mutations).
static CONFIG_CACHE: OnceLock<RwLock<Option<LynxConfig>>> = OnceLock::new();

fn cache() -> &'static RwLock<Option<LynxConfig>> {
    CONFIG_CACHE.get_or_init(|| RwLock::new(None))
}

/// Invalidate the in-process config cache. Must be called after any mutation to config on disk.
pub fn invalidate_cache() {
    if let Ok(mut guard) = cache().write() {
        *guard = None;
    }
}

/// Resolve config file path: `$HOME/.config/lynx/config.toml`.
pub fn config_path() -> PathBuf {
    lynx_core::paths::config_file()
}

/// Load config, returning a cached clone if available. Falls back to disk on first call.
/// Returns `LynxConfig::default()` if the file is missing. Applies migration if schema_version is stale.
pub fn load() -> Result<LynxConfig> {
    // Fast path: return cached value if present.
    if let Ok(guard) = cache().read() {
        if let Some(cfg) = guard.as_ref() {
            return Ok(cfg.clone());
        }
    }
    // Slow path: read from disk, populate cache.
    let cfg = load_from(&config_path())?;
    if let Ok(mut guard) = cache().write() {
        *guard = Some(cfg.clone());
    }
    Ok(cfg)
}

pub fn load_from(path: &Path) -> Result<LynxConfig> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let mut cfg: LynxConfig =
                toml::from_str(&content).map_err(|e| LynxError::Config(e.to_string()))?;
            migrate::migrate(&mut cfg)?;
            Ok(cfg)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LynxConfig::default()),
        Err(e) => Err(LynxError::IoRaw(e)),
    }
}

/// Enable a plugin by adding it to enabled_plugins (D-007: snapshot → validate → apply).
pub fn enable_plugin(name: &str) -> Result<()> {
    let config = load()?;
    if config.enabled_plugins.iter().any(|p| p == name) {
        return Ok(());
    }
    let result = snapshot::mutate_config_transaction(&format!("plugin-enable-{name}"), |cfg| {
        cfg.enabled_plugins.push(name.to_string());
        Ok(())
    });
    if result.is_ok() {
        invalidate_cache();
    }
    result
}

/// Disable a plugin by removing it from enabled_plugins (D-007: snapshot → validate → apply).
pub fn disable_plugin(name: &str) -> Result<()> {
    let config = load()?;
    if !config.enabled_plugins.iter().any(|p| p == name) {
        return Err(LynxError::Config(format!("plugin '{name}' is not enabled")));
    }
    let result = snapshot::mutate_config_transaction(&format!("plugin-disable-{name}"), |cfg| {
        cfg.enabled_plugins.retain(|p| p != name);
        Ok(())
    });
    if result.is_ok() {
        invalidate_cache();
    }
    result
}

/// Validate then write config to disk (D-007: validate before writing).
pub fn save(config: &LynxConfig) -> Result<()> {
    let result = save_to(config, &config_path());
    if result.is_ok() {
        invalidate_cache();
    }
    result
}

pub fn save_to(config: &LynxConfig, path: &Path) -> Result<()> {
    validate::validate_before_apply(config)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(LynxError::IoRaw)?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| LynxError::Config(e.to_string()))?;
    std::fs::write(path, content).map_err(LynxError::IoRaw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_core::types::Context;
    use schema::CURRENT_SCHEMA_VERSION;

    #[test]
    fn missing_file_returns_defaults() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().with_extension("toml");
        // path does not exist yet
        let cfg = load_from(&path).unwrap();
        assert_eq!(cfg, LynxConfig::default());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = LynxConfig {
            active_theme: "nord".into(),
            active_context: Context::Agent,
            enabled_plugins: vec!["git".into()],
            ..Default::default()
        };
        save_to(&cfg, &path).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(cfg, loaded);
    }

    #[test]
    fn stale_schema_version_migrates_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        // Write a config with stale version
        std::fs::write(
            &path,
            r#"schema_version = 0
active_theme = "default"
active_context = "interactive"
enabled_plugins = []
"#,
        )
        .unwrap();
        let cfg = load_from(&path).unwrap();
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn invalidate_cache_clears_cached_value() {
        // Warm the cache with a known value via load_from, then manually prime.
        if let Ok(mut guard) = cache().write() {
            *guard = Some(LynxConfig {
                active_theme: "cached".into(),
                ..Default::default()
            });
        }
        // Cache is warm.
        assert!(cache().read().unwrap().is_some());
        // Invalidating clears it.
        invalidate_cache();
        assert!(cache().read().unwrap().is_none());
    }

    #[test]
    fn save_rejects_empty_theme() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = LynxConfig {
            active_theme: "".into(),
            ..Default::default()
        };
        assert!(save_to(&cfg, &path).is_err());
    }
}
