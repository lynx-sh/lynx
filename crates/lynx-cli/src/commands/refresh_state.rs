use anyhow::Result;
use clap::Args;

use super::git;
use super::kubectl_state;

#[derive(Args)]
pub struct RefreshStateArgs {}

/// `lx refresh-state` — gather state for all enabled plugins concurrently and
/// emit zsh that updates every plugin's cache in one eval call.
///
/// Registered as the single `_lynx_precmd` hook by `lx init`. Replaces the
/// old per-plugin hook pattern where each plugin spawned its own `lx` process
/// on every precmd — with N plugins that was N spawns per command typed.
///
/// Now: always 1 spawn regardless of how many plugins are enabled.
///
/// Called from shell/core/hooks.zsh:
/// ```zsh
/// _lynx_hook_precmd() {
///   eval "$(lx refresh-state 2>/dev/null)"
///   eval "$(lx prompt render 2>/dev/null)"
/// }
/// ```
pub async fn run(_args: RefreshStateArgs) -> Result<()> {
    let enabled = read_enabled_plugins();
    let output = gather_all(&enabled);
    print!("{}", output);
    Ok(())
}

/// Read the enabled plugin list from config, falling back to empty on any error.
fn read_enabled_plugins() -> Vec<String> {
    lynx_config::load_from(&lynx_core::paths::config_file())
        .ok()
        .map(|cfg| cfg.enabled_plugins)
        .unwrap_or_default()
}

/// Gather state for all enabled plugins. Each gatherer runs independently;
/// failures are silently skipped (plugin not installed / binary missing).
///
/// Returns concatenated zsh output ready for eval.
fn gather_all(enabled: &[String]) -> String {
    let mut out = String::new();

    // Run gatherers for known state-bearing plugins.
    // Order is stable: git before kubectl (alphabetical within category).
    if enabled.iter().any(|p| p == "git") {
        out.push_str(&git::render_zsh(&git::gather_git_state()));
    }

    if enabled.iter().any(|p| p == "kubectl") {
        out.push_str(&kubectl_state::render_zsh(&kubectl_state::gather_kubectl_state()));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_plugin_list_emits_nothing() {
        let out = gather_all(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn unknown_plugin_is_silently_skipped() {
        let out = gather_all(&["nonexistent-plugin".to_string()]);
        assert!(out.is_empty());
    }

    #[test]
    fn git_plugin_emits_git_state() {
        // git is always available in CI (we run git tests elsewhere too)
        let out = gather_all(&["git".to_string()]);
        // Will either have state (if in a git repo) or clear state
        assert!(out.contains("_lynx_git_state="));
        assert!(out.contains("LYNX_CACHE_GIT_STATE"));
    }

    #[test]
    fn kubectl_plugin_emits_kubectl_state() {
        let out = gather_all(&["kubectl".to_string()]);
        // kubectl may not be installed — either way we get a clear or populated state
        assert!(out.contains("_lynx_kubectl_state="));
        assert!(out.contains("LYNX_CACHE_KUBECTL_STATE"));
    }

    #[test]
    fn multiple_plugins_emit_all_states() {
        let out = gather_all(&["git".to_string(), "kubectl".to_string()]);
        assert!(out.contains("_lynx_git_state="));
        assert!(out.contains("_lynx_kubectl_state="));
    }
}
