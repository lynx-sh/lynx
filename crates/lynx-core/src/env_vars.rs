//! Single source of truth for every environment variable name Lynx reads or writes.
//!
//! Always reference these constants instead of spelling variable names inline.
//! This prevents typos, makes grep-based audits reliable, and gives AI agents
//! one authoritative place to look up any env var name.

// ── Lynx runtime env vars ────────────────────────────────────────────────────

/// Set by `lx init` to the detected context value. Read back by all context-aware code.
pub const LYNX_CONTEXT: &str = "LYNX_CONTEXT";

/// Overrides the Lynx installation directory (default: `~/.config/lynx`).
pub const LYNX_DIR: &str = "LYNX_DIR";

/// Set by `lx init` to the plugin directory path.
pub const LYNX_PLUGIN_DIR: &str = "LYNX_PLUGIN_DIR";

/// Idempotency guard — set after `lx init` completes; unset before each new shell init.
pub const LYNX_INITIALIZED: &str = "LYNX_INITIALIZED";

/// Overrides the runtime directory used for sockets and PID files.
pub const LYNX_RUNTIME_DIR: &str = "LYNX_RUNTIME_DIR";

/// Overrides the daemon binary path (used by launchd/systemd service installers).
pub const LYNX_DAEMON_BIN: &str = "LYNX_DAEMON_BIN";

/// Controls log verbosity for the daemon (e.g. `debug`, `info`, `warn`, `error`).
pub const LYNX_LOG: &str = "LYNX_LOG";

// ── Agent detection env vars ─────────────────────────────────────────────────

/// Set to `"1"` by Claude Code in every shell it spawns.
pub const CLAUDECODE: &str = "CLAUDECODE";

/// Set by Cursor in its integrated terminal.
pub const CURSOR_CLI: &str = "CURSOR_CLI";

/// Set to `"true"` by CI systems (GitHub Actions, CircleCI, etc.).
pub const CI: &str = "CI";

/// XDG standard runtime directory — used as fallback for runtime_dir resolution.
pub const XDG_RUNTIME_DIR: &str = "XDG_RUNTIME_DIR";

/// Shell `$HOME` — used for all config/data path derivation.
pub const HOME: &str = "HOME";

/// Returns the canonical plugin load-guard variable name for a given plugin name.
///
/// Pattern: `LYNX_PLUGIN_{NAME_UPPERCASE_UNDERSCORED}_LOADED`
///
/// This is the single source of truth for the guard variable format used in both
/// `lx init` (clearing inherited guards) and `lx plugin exec` (unload scripts).
pub fn plugin_guard_var(plugin_name: &str) -> String {
    format!(
        "LYNX_PLUGIN_{}_LOADED",
        plugin_name.to_uppercase().replace('-', "_")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_guard_var_normalizes_hyphens() {
        assert_eq!(plugin_guard_var("my-plugin"), "LYNX_PLUGIN_MY_PLUGIN_LOADED");
    }

    #[test]
    fn plugin_guard_var_uppercase() {
        assert_eq!(plugin_guard_var("git"), "LYNX_PLUGIN_GIT_LOADED");
    }
}
