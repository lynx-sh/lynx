use anyhow::Result;
use clap::Args;
use std::process::{Command, Stdio};

use super::git;
use super::kubectl_state;

#[derive(Args)]
pub struct RefreshStateArgs {}

/// `lx refresh-state` — gather state for all enabled plugins in one eval call.
///
/// Registered as the single precmd hook by `lx init`. One spawn regardless of
/// how many plugins are enabled.
///
/// Two gatherer paths (D-014):
/// - First-party plugins (git, kubectl): native Rust gatherers inside lx — fastest path.
/// - Community plugins: `state.gather` command declared in plugin.toml, called via
///   shell and evaled. May be written in any language.
///
/// Called from shell/core/hooks.zsh:
/// ```zsh
/// _lynx_hook_precmd() {
///   eval "$(lx refresh-state 2>/dev/null)"
///   eval "$(lx prompt render 2>/dev/null)"
/// }
/// ```
pub fn run(_args: RefreshStateArgs) -> Result<()> {
    let enabled = read_enabled_plugins();
    let output = gather_all(&enabled);
    print!("{output}");
    Ok(())
}

/// Read the enabled plugin list from config, falling back to empty on any error.
fn read_enabled_plugins() -> Vec<String> {
    match lynx_config::load_from(&lynx_core::paths::config_file()) {
        Ok(cfg) => cfg.enabled_plugins,
        Err(e) => {
            lynx_core::diag::warn(
                "refresh-state",
                &format!("failed to load config — plugins will not load: {e}"),
            );
            Vec::new()
        }
    }
}

/// Gather state for all enabled plugins. Failures are silently skipped.
/// Returns concatenated zsh output ready for eval.
/// Check if the current user is root (uid 0) via `id -u`.
fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .map(|uid| uid == "0")
        .unwrap_or(false)
}

fn gather_all(enabled: &[String]) -> String {
    let mut out = String::new();

    // Emit LYNX_USER_IS_ROOT — computed in Rust, not in shell (D-001).
    let root_val = if is_root() { "1" } else { "0" };
    out.push_str(&format!("export LYNX_USER_IS_ROOT={root_val}\n"));

    for plugin_name in enabled {
        match plugin_name.as_str() {
            // First-party: native Rust gatherers — no extra process spawn.
            "git" => out.push_str(&git::render_zsh(&git::gather_git_state())),
            "kubectl" => out.push_str(&kubectl_state::render_zsh(
                &kubectl_state::gather_kubectl_state(),
            )),
            // Community: look for state.gather in plugin.toml.
            name => {
                if let Some(zsh) = gather_community_plugin(name) {
                    out.push_str(&zsh);
                }
            }
        }
    }

    out
}

/// Read a community plugin's plugin.toml, run its `state.gather` command if set,
/// and return its stdout for eval. Returns None if no gather command or on failure.
fn gather_community_plugin(plugin_name: &str) -> Option<String> {
    let plugin_dir = lynx_core::paths::installed_plugins_dir().join(plugin_name);
    let manifest_path = plugin_dir.join(lynx_core::brand::PLUGIN_MANIFEST);
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let manifest = lynx_manifest::parse(&content).ok()?;
    let gather_cmd = manifest.state.gather.as_deref().filter(|s| !s.is_empty())?;

    // Expand $PLUGIN_DIR in the command so plugins can reference their own files.
    let cmd = gather_cmd.replace("$PLUGIN_DIR", &plugin_dir.to_string_lossy());

    let output = Command::new("zsh")
        .args(["-c", &cmd])
        .env("PLUGIN_DIR", &plugin_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_test_utils::env_lock;

    struct LynxDirGuard(Option<String>);

    impl LynxDirGuard {
        fn new() -> Self {
            Self(std::env::var(lynx_core::env_vars::LYNX_DIR).ok())
        }
    }

    impl Drop for LynxDirGuard {
        fn drop(&mut self) {
            if let Some(v) = &self.0 {
                std::env::set_var(lynx_core::env_vars::LYNX_DIR, v);
            } else {
                std::env::remove_var(lynx_core::env_vars::LYNX_DIR);
            }
        }
    }

    #[test]
    fn empty_plugin_list_emits_root_status_only() {
        let out = gather_all(&[]);
        assert!(
            out.contains("LYNX_USER_IS_ROOT="),
            "expected root status: {out}"
        );
        // Should not contain any plugin state.
        assert!(
            !out.contains("_lynx_git_state"),
            "should have no git state: {out}"
        );
    }

    #[test]
    fn unknown_community_plugin_without_manifest_is_silently_skipped() {
        // Plugin has no installed directory — should not panic or error.
        let out = gather_all(&["nonexistent-plugin".to_string()]);
        // Only root status, no plugin state.
        assert!(
            out.contains("LYNX_USER_IS_ROOT="),
            "expected root status: {out}"
        );
        assert!(
            !out.contains("_lynx_git_state"),
            "should have no git state: {out}"
        );
    }

    #[test]
    fn git_first_party_emits_git_state() {
        let out = gather_all(&["git".to_string()]);
        assert!(out.contains("_lynx_git_state="));
        assert!(out.contains("LYNX_CACHE_GIT_STATE"));
    }

    #[test]
    fn kubectl_first_party_emits_kubectl_state() {
        let out = gather_all(&["kubectl".to_string()]);
        assert!(out.contains("_lynx_kubectl_state="));
        assert!(out.contains("LYNX_CACHE_KUBECTL_STATE"));
    }

    #[test]
    fn multiple_first_party_plugins_emit_all_states() {
        let out = gather_all(&["git".to_string(), "kubectl".to_string()]);
        assert!(out.contains("_lynx_git_state="));
        assert!(out.contains("_lynx_kubectl_state="));
    }

    #[test]
    fn community_plugin_state_gather_is_called_and_evaled() {
        use std::fs;
        let _lock = env_lock().lock().expect("lock");
        let _guard = LynxDirGuard::new();
        let tmp = tempfile::tempdir().expect("tempdir");

        // Build a fake installed plugin directory with plugin.toml declaring state.gather
        let plugin_dir = tmp.path().join("plugins").join("myplugin");
        fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name    = "myplugin"
version = "0.1.0"

[state]
gather = "echo \"export LYNX_CACHE_MYPLUGIN_STATE='test'\""
"#,
        )
        .expect("write plugin.toml");

        // Override LYNX_DIR so installed_plugins_dir() resolves to our tmp dir.
        std::env::set_var(lynx_core::env_vars::LYNX_DIR, tmp.path());
        let out = gather_community_plugin("myplugin");

        let out = out.expect("should produce output");
        assert!(out.contains("LYNX_CACHE_MYPLUGIN_STATE"), "got: {out}");
    }

    #[test]
    fn community_plugin_without_state_gather_is_skipped() {
        use std::fs;
        let _lock = env_lock().lock().expect("lock");
        let _guard = LynxDirGuard::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugin_dir = tmp.path().join("plugins").join("bare");
        fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"[plugin]
name    = "bare"
version = "0.1.0"
"#,
        )
        .expect("write plugin.toml");

        std::env::set_var(lynx_core::env_vars::LYNX_DIR, tmp.path());
        let out = gather_community_plugin("bare");

        assert!(out.is_none(), "no state.gather should yield None");
    }
}
