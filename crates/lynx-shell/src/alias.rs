use lynx_config::schema::{AliasContext, LynxConfig, UserAlias};
use lynx_config::snapshot::mutate_config_transaction;
use lynx_core::error::{LynxError, Result};
use std::path::Path;
use tracing::warn;

/// Source of a resolved alias — either defined by the user or provided by a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliasSrc {
    User,
    Plugin(String),
}

/// A resolved alias combining user-defined and plugin-provided aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAlias {
    pub name: String,
    pub command: String,
    pub context: AliasContext,
    pub description: Option<String>,
    pub source: AliasSrc,
}

/// Add a user-defined alias to config.
///
/// Returns an error if the name already exists in user aliases.
/// Warns (but does not block) if the name shadows a known plugin alias.
pub fn add_alias(alias: UserAlias, plugin_dir: &Path) -> Result<()> {
    // Check for plugin name collision and warn — does not block.
    let plugin_names = collect_plugin_alias_names(plugin_dir);
    if plugin_names.iter().any(|n| n == &alias.name) {
        warn!(
            "alias '{}' shadows a plugin-provided alias — the user alias will take precedence",
            alias.name
        );
    }

    mutate_config_transaction("alias-add", |cfg| {
        if cfg.aliases.iter().any(|a| a.name == alias.name) {
            return Err(LynxError::Config(format!(
                "alias '{}' already exists — run `lx alias remove {}` first, then re-add it",
                alias.name, alias.name
            )));
        }
        cfg.aliases.push(alias);
        Ok(())
    })
}

/// Remove a user-defined alias by name.
///
/// Returns `LynxError::NotFound` if the alias does not exist.
pub fn remove_alias(name: &str) -> Result<()> {
    mutate_config_transaction("alias-remove", |cfg| {
        let before = cfg.aliases.len();
        cfg.aliases.retain(|a| a.name != name);
        if cfg.aliases.len() == before {
            return Err(LynxError::NotFound {
                item_type: "alias".into(),
                name: name.to_string(),
                hint: "run `lx alias list` to see all defined aliases".into(),
            });
        }
        Ok(())
    })
}

/// List all active aliases — merges user aliases from config with plugin-provided aliases.
///
/// User aliases are listed first; plugin aliases follow. If a user alias shadows a plugin
/// alias by name, only the user alias appears (user takes precedence).
pub fn list_aliases(cfg: &LynxConfig, plugin_dir: &Path) -> Vec<ResolvedAlias> {
    let mut resolved: Vec<ResolvedAlias> = cfg
        .aliases
        .iter()
        .map(|a| ResolvedAlias {
            name: a.name.clone(),
            command: a.command.clone(),
            context: a.context.clone(),
            description: a.description.clone(),
            source: AliasSrc::User,
        })
        .collect();

    let user_names: std::collections::HashSet<String> =
        resolved.iter().map(|a| a.name.clone()).collect();

    // Merge plugin aliases, skipping any shadowed by user aliases.
    for entry in collect_plugin_aliases(plugin_dir) {
        if !user_names.contains(&entry.name) {
            resolved.push(entry);
        }
    }

    resolved
}

// ── Internal helpers ───────────────────────────────────────────────────────

/// Collect all alias names declared in installed plugin manifests.
fn collect_plugin_alias_names(plugin_dir: &Path) -> Vec<String> {
    collect_plugin_aliases(plugin_dir)
        .into_iter()
        .map(|a| a.name)
        .collect()
}

/// Collect all aliases declared in installed plugin manifests with their metadata.
fn collect_plugin_aliases(plugin_dir: &Path) -> Vec<ResolvedAlias> {
    let mut out = Vec::new();

    let Ok(entries) = std::fs::read_dir(plugin_dir) else {
        return out;
    };

    for entry in entries.flatten() {
        let manifest_path = entry.path().join(lynx_core::brand::PLUGIN_MANIFEST);
        let Ok(content) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = lynx_manifest::parse(&content) else {
            continue;
        };
        let plugin_name = manifest.plugin.name.clone();
        for alias_name in manifest.exports.aliases {
            out.push(ResolvedAlias {
                name: alias_name,
                command: String::new(), // plugin aliases resolve at shell load time
                context: AliasContext::Interactive,
                description: None,
                source: AliasSrc::Plugin(plugin_name.clone()),
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Serialise tests that mutate LYNX_DIR so they don't race each other.
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    /// Set LYNX_DIR to `dir`, write an empty config.toml, and return the lock guard.
    /// The guard must be kept alive for the duration of the test.
    fn isolated_lynx_dir(dir: &TempDir) -> std::sync::MutexGuard<'static, ()> {
        let guard = env_lock().lock().unwrap();
        std::fs::write(dir.path().join("config.toml"), "active_theme = \"default\"").unwrap();
        std::env::set_var(lynx_core::env_vars::LYNX_DIR, dir.path());
        guard
    }

    fn make_alias(name: &str, command: &str) -> UserAlias {
        UserAlias {
            name: name.to_string(),
            command: command.to_string(),
            description: None,
            context: AliasContext::Interactive,
        }
    }

    fn empty_plugin_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    // ── add_alias ─────────────────────────────────────────────────────────

    #[test]
    fn add_alias_duplicate_name_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = isolated_lynx_dir(&dir);

        let pd = empty_plugin_dir();
        add_alias(make_alias("gs", "git status"), pd.path()).unwrap();
        let err = add_alias(make_alias("gs", "git stash"), pd.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("gs"), "error should name the alias: {msg}");
    }

    #[test]
    fn add_alias_plugin_collision_warns_but_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = isolated_lynx_dir(&dir);

        // Create a minimal plugin with an alias declaration.
        let plugin_dir = tempfile::tempdir().unwrap();
        let p = plugin_dir.path().join("git");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("plugin.toml"),
            r#"[plugin]
name = "git"
version = "0.1.0"
description = "git helpers"

[exports]
functions = []
aliases = ["gs"]
"#,
        )
        .unwrap();

        // Should succeed even though "gs" is a plugin alias.
        let result = add_alias(make_alias("gs", "git status --short"), plugin_dir.path());
        assert!(result.is_ok(), "plugin collision should warn but not block");
    }

    // ── remove_alias ──────────────────────────────────────────────────────

    #[test]
    fn remove_alias_unknown_name_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = isolated_lynx_dir(&dir);

        let err = remove_alias("nonexistent").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent"),
            "error should name the missing alias: {msg}"
        );
    }

    // ── list_aliases ──────────────────────────────────────────────────────

    #[test]
    fn list_aliases_no_plugins_returns_only_user_aliases() {
        let cfg = LynxConfig {
            aliases: vec![make_alias("ll", "ls -la")],
            ..Default::default()
        };
        let pd = empty_plugin_dir();
        let result = list_aliases(&cfg, pd.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "ll");
        assert_eq!(result[0].source, AliasSrc::User);
    }

    #[test]
    fn list_aliases_merges_plugin_aliases() {
        // Plugin with alias "gs".
        let plugin_dir = tempfile::tempdir().unwrap();
        let p = plugin_dir.path().join("git");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("plugin.toml"),
            r#"[plugin]
name = "git"
version = "0.1.0"
description = "git helpers"

[exports]
functions = []
aliases = ["gs"]
"#,
        )
        .unwrap();

        let cfg = LynxConfig::default();
        let result = list_aliases(&cfg, plugin_dir.path());
        assert!(result.iter().any(|a| a.name == "gs"));
        assert!(result
            .iter()
            .any(|a| a.source == AliasSrc::Plugin("git".into())));
    }

    #[test]
    fn list_aliases_user_alias_shadows_plugin_alias() {
        let plugin_dir = tempfile::tempdir().unwrap();
        let p = plugin_dir.path().join("git");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("plugin.toml"),
            r#"[plugin]
name = "git"
version = "0.1.0"
description = "git helpers"

[exports]
functions = []
aliases = ["gs"]
"#,
        )
        .unwrap();

        let cfg = LynxConfig {
            aliases: vec![make_alias("gs", "git status --short")],
            ..Default::default()
        };
        let result = list_aliases(&cfg, plugin_dir.path());

        // Only one "gs" — the user alias wins.
        let gs_entries: Vec<_> = result.iter().filter(|a| a.name == "gs").collect();
        assert_eq!(gs_entries.len(), 1);
        assert_eq!(gs_entries[0].source, AliasSrc::User);
    }
}
