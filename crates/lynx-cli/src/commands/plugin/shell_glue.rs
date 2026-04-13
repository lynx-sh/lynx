// Shell eval-bridge helpers for plugin exec and unload.
//
// These functions generate zsh that is eval'd by the shell via the eval-bridge pattern.
// All output goes to stdout — the shell captures it with $().
// Never write to stderr here; the shell's 2>/dev/null suppresses it on load.

use anyhow::Result;
use lynx_events::types::{Event, PLUGIN_LOADED};
use lynx_manifest::schema::PluginManifest;
use lynx_plugin::{exec::generate_exec_script, lifecycle};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Emit zsh that activates the plugin's exported symbols and sets its load guard.
///
/// After emitting the shell glue, activates the plugin's EventBus subscriptions
/// in-process and emits plugin:loaded so any registered handlers run.
pub(super) async fn cmd_exec(name: &str) -> Result<()> {
    let script = generate_exec_script_for_plugin(name)?;
    print!("{script}");

    // Activate in-process: register this plugin's hook subscriptions on a
    // short-lived bus, emit plugin:loaded, then exit.
    if let Some(plugin_dir) = resolve_plugin_dir(name) {
        if let Ok(Some(manifest)) = read_plugin_manifest(&plugin_dir) {
            let bus = Arc::new(lynx_events::EventBus::new());
            if let Err(e) = lifecycle::activate(name, &manifest, Arc::clone(&bus)) {
                tracing::warn!("plugin '{name}' activation failed: {e}");
            }
            bus.emit(Event::new(PLUGIN_LOADED, name)).await;
        }
    }

    Ok(())
}

/// Emit zsh that removes the plugin's exported symbols and clears its load guard.
pub(super) async fn cmd_unload(name: &str) -> Result<()> {
    let script = generate_unload_script_for_plugin(name)?;
    print!("{script}");
    Ok(())
}

fn generate_exec_script_for_plugin(name: &str) -> Result<String> {
    let resolved_dir = resolve_plugin_dir(name)
        .ok_or_else(|| anyhow::Error::from(lynx_core::error::LynxError::NotFound {
            item_type: "Plugin".into(),
            name: name.to_string(),
            hint: "run `lx doctor` to diagnose".into(),
        }))?;

    let manifest = read_plugin_manifest(&resolved_dir)?
        .ok_or_else(|| anyhow::Error::from(lynx_core::error::LynxError::Manifest(format!("plugin '{name}' has no plugin.toml"))))?;

    generate_exec_script(&manifest, &resolved_dir).map_err(|e| anyhow::Error::from(lynx_core::error::LynxError::Plugin(e.to_string())))
}

fn generate_unload_script_for_plugin(name: &str) -> Result<String> {
    let resolved_dir = resolve_plugin_dir(name);
    let manifest = match resolved_dir {
        Some(dir) => read_plugin_manifest(&dir)?,
        None => None,
    };
    Ok(build_unload_script(name, manifest.as_ref()))
}

/// Resolve the plugin directory for the given name.
///
/// Checks the installed plugins dir first, then the in-repo plugins/ directory
/// (used during development and tests).
fn resolve_plugin_dir(name: &str) -> Option<PathBuf> {
    let installed = lynx_core::paths::installed_plugins_dir().join(name);
    if installed.exists() {
        return Some(installed);
    }

    let repo = PathBuf::from("plugins").join(name);
    if repo.exists() {
        return Some(repo);
    }

    None
}

pub(super) fn read_plugin_manifest(plugin_dir: &Path) -> Result<Option<PluginManifest>> {
    let manifest_path = plugin_dir.join(lynx_core::brand::PLUGIN_MANIFEST);
    if !manifest_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest = lynx_manifest::parse(&content).map_err(|e| anyhow::Error::from(lynx_core::error::LynxError::Manifest(e.to_string())))?;
    Ok(Some(manifest))
}

fn build_unload_script(name: &str, manifest: Option<&PluginManifest>) -> String {
    let guard_var = lynx_core::env_vars::plugin_guard_var(name);
    let mut out = String::new();

    if let Some(manifest) = manifest {
        for func in &manifest.exports.functions {
            out.push_str(&format!("unfunction {func} 2>/dev/null\n"));
        }
        for alias in &manifest.exports.aliases {
            out.push_str(&format!("unalias {alias} 2>/dev/null\n"));
        }
        for hook in &manifest.load.hooks {
            let fn_name = format!("_{}_plugin_{}", name.replace('-', "_"), hook);
            out.push_str(&format!(
                "add-zsh-hook -d {hook} {fn_name} 2>/dev/null\n"
            ));
        }
    }

    out.push_str(&format!("unset {guard_var}\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_test_utils::{fixture_plugin, temp_home};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        old_lynx_dir: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                old_lynx_dir: std::env::var(lynx_core::env_vars::LYNX_DIR).ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(v) = &self.old_lynx_dir {
                std::env::set_var(lynx_core::env_vars::LYNX_DIR, v);
            } else {
                std::env::remove_var(lynx_core::env_vars::LYNX_DIR);
            }
        }
    }

    #[test]
    fn build_unload_script_with_manifest_unloads_all_exports_and_hooks() {
        let git_plugin = fixture_plugin("git");
        let manifest = read_plugin_manifest(&git_plugin)
            .expect("manifest read")
            .expect("manifest present");
        let script = build_unload_script("git", Some(&manifest));
        assert!(script.contains("unfunction git_branch 2>/dev/null"));
        assert!(script.contains("unalias gst 2>/dev/null"));
        // git plugin no longer self-registers hooks — lx refresh-state handles precmd
        assert!(!script.contains("add-zsh-hook"));
        assert!(script.contains("unset LYNX_PLUGIN_GIT_LOADED"));
    }

    #[test]
    fn build_unload_script_without_manifest_only_unsets_guard() {
        let script = build_unload_script("missing-plugin", None);
        assert!(script.contains("unset LYNX_PLUGIN_MISSING_PLUGIN_LOADED"));
        assert!(!script.contains("unfunction"));
        assert!(!script.contains("unalias"));
    }

    #[test]
    fn resolve_plugin_dir_finds_installed_plugin() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new();
        let temp = temp_home();
        // installed_plugins_dir() = LYNX_DIR/plugins — set LYNX_DIR to temp root
        std::env::set_var(lynx_core::env_vars::LYNX_DIR, temp.path());
        let plugin_path = temp.path().join("plugins").join("demo");
        std::fs::create_dir_all(&plugin_path).expect("create plugin path");
        let resolved = resolve_plugin_dir("demo");
        assert_eq!(resolved, Some(plugin_path));
    }

    #[test]
    fn read_plugin_manifest_returns_none_when_missing() {
        let temp = temp_home();
        let manifest = read_plugin_manifest(temp.path()).expect("read result");
        assert!(manifest.is_none());
    }

    #[test]
    fn generate_exec_script_for_plugin_contains_hook_wiring() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new();
        let git_plugin = fixture_plugin("git");
        let plugins_dir = git_plugin.parent().expect("plugins dir parent");
        let lynx_dir = plugins_dir.parent().expect("lynx dir parent");
        std::env::set_var(lynx_core::env_vars::LYNX_DIR, lynx_dir);

        let script = generate_exec_script_for_plugin("git").expect("exec script");
        // git plugin no longer self-registers hooks — lx refresh-state handles precmd
        assert!(!script.contains("add-zsh-hook"));
        assert!(script.contains("LYNX_PLUGIN_GIT_LOADED"));
    }

    #[test]
    fn generate_exec_script_for_missing_plugin_errors() {
        let err = generate_exec_script_for_plugin("definitely-missing-plugin");
        assert!(err.is_err());
    }
}
