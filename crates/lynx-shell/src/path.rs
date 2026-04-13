use lynx_config::schema::{LynxConfig, UserPath};
use lynx_config::snapshot::mutate_config_transaction;
use lynx_core::error::{LynxError, Result};

/// Source of a resolved PATH entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSrc {
    /// Defined by the user in config.
    User,
    /// Set at shell init time (not managed by Lynx).
    Init,
}

/// A resolved PATH entry with source annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPath {
    pub path: String,
    pub label: Option<String>,
    pub source: PathSrc,
}

/// Add a user-defined PATH entry to config.
///
/// Returns an error if the path string already exists in user paths.
pub fn add_path(entry: UserPath) -> Result<()> {
    mutate_config_transaction("path-add", |cfg| {
        if cfg.paths.iter().any(|p| p.path == entry.path) {
            return Err(LynxError::Config(format!(
                "path '{}' already exists — run `lx path remove '{}'` first if you want to update the label",
                entry.path, entry.path
            )));
        }
        cfg.paths.push(entry);
        Ok(())
    })
}

/// Remove a user-defined PATH entry by path string.
///
/// Returns `LynxError::NotFound` if the path does not exist in user config.
pub fn remove_path(path_str: &str) -> Result<()> {
    mutate_config_transaction("path-remove", |cfg| {
        let before = cfg.paths.len();
        cfg.paths.retain(|p| p.path != path_str);
        if cfg.paths.len() == before {
            return Err(LynxError::NotFound {
                item_type: "path".into(),
                name: path_str.to_string(),
                hint: "run `lx path list` to see all managed paths".into(),
            });
        }
        Ok(())
    })
}

/// List all user-managed PATH entries from config.
///
/// All entries are sourced from user config (`PathSrc::User`).
/// Paths inherited from the system or init scripts are not enumerated here.
pub fn list_paths(cfg: &LynxConfig) -> Vec<ResolvedPath> {
    cfg.paths
        .iter()
        .map(|p| ResolvedPath {
            path: p.path.clone(),
            label: p.label.clone(),
            source: PathSrc::User,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn isolated_lynx_dir(dir: &TempDir) -> std::sync::MutexGuard<'static, ()> {
        let guard = env_lock().lock().unwrap();
        std::fs::write(dir.path().join("config.toml"), "active_theme = \"default\"").unwrap();
        std::env::set_var(lynx_core::env_vars::LYNX_DIR, dir.path());
        guard
    }

    fn make_path(path: &str) -> UserPath {
        UserPath {
            path: path.to_string(),
            label: None,
        }
    }

    // ── add_path ──────────────────────────────────────────────────────────

    #[test]
    fn add_path_duplicate_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = isolated_lynx_dir(&dir);

        add_path(make_path("/usr/local/sbin")).unwrap();
        let err = add_path(make_path("/usr/local/sbin")).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("/usr/local/sbin"),
            "error should name the duplicate path: {msg}"
        );
    }

    // ── remove_path ───────────────────────────────────────────────────────

    #[test]
    fn remove_path_unknown_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = isolated_lynx_dir(&dir);

        let err = remove_path("/nonexistent/path").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("/nonexistent/path"),
            "error should name the missing path: {msg}"
        );
    }

    // ── list_paths ────────────────────────────────────────────────────────

    #[test]
    fn list_paths_returns_correct_source_annotation() {
        let cfg = LynxConfig {
            paths: vec![
                UserPath {
                    path: "/usr/local/sbin".into(),
                    label: Some("sbin".into()),
                },
                UserPath {
                    path: "/opt/homebrew/bin".into(),
                    label: None,
                },
            ],
            ..Default::default()
        };

        let result = list_paths(&cfg);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.source == PathSrc::User));
        assert_eq!(result[0].path, "/usr/local/sbin");
        assert_eq!(result[0].label.as_deref(), Some("sbin"));
        assert_eq!(result[1].path, "/opt/homebrew/bin");
        assert!(result[1].label.is_none());
    }

    #[test]
    fn list_paths_empty_config_returns_empty() {
        let cfg = LynxConfig::default();
        assert!(list_paths(&cfg).is_empty());
    }
}
