pub mod defaults;
pub mod migrate;
pub mod schema;

use lynx_core::error::{LynxError, Result};
use schema::LynxConfig;
use std::path::{Path, PathBuf};

/// Resolve config file path: `$HOME/.config/lynx/config.toml`.
pub fn config_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/lynx/config.toml")
}

/// Load config from disk. Returns `LynxConfig::default()` if the file is missing.
/// Applies migration if schema_version is stale.
pub fn load() -> Result<LynxConfig> {
    load_from(&config_path())
}

pub fn load_from(path: &Path) -> Result<LynxConfig> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let mut cfg: LynxConfig = toml::from_str(&content)
                .map_err(|e| LynxError::Config(e.to_string()))?;
            migrate::migrate(&mut cfg)?;
            Ok(cfg)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LynxConfig::default()),
        Err(e) => Err(LynxError::Io(e)),
    }
}

/// Validate then write config to disk (D-007: validate before writing).
pub fn save(config: &LynxConfig) -> Result<()> {
    save_to(config, &config_path())
}

pub fn save_to(config: &LynxConfig, path: &Path) -> Result<()> {
    validate(config)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(LynxError::Io)?;
    }
    let content = toml::to_string_pretty(config)
        .map_err(|e| LynxError::Config(e.to_string()))?;
    std::fs::write(path, content).map_err(LynxError::Io)
}

fn validate(config: &LynxConfig) -> Result<()> {
    if config.active_theme.is_empty() {
        return Err(LynxError::Config("active_theme must not be empty".into()));
    }
    Ok(())
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
