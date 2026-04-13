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

/// Overrides the active theme name. Read by `lx prompt render`.
pub const LYNX_THEME: &str = "LYNX_THEME";

/// Duration of the last shell command in milliseconds. Set by the precmd hook.
pub const LYNX_LAST_CMD_MS: &str = "LYNX_LAST_CMD_MS";

/// JSON-serialized git state cache. Set by the git plugin's chpwd/precmd hook.
pub const LYNX_CACHE_GIT_STATE: &str = "LYNX_CACHE_GIT_STATE";

/// JSON-serialized kubectl context cache. Set by the kubectl plugin.
pub const LYNX_CACHE_KUBECTL_STATE: &str = "LYNX_CACHE_KUBECTL_STATE";

/// JSON-serialized Node.js version cache. Set by the node plugin from .node-version/.nvmrc.
pub const LYNX_CACHE_NODE_STATE: &str = "LYNX_CACHE_NODE_STATE";

/// JSON-serialized Ruby version cache. Set by the ruby plugin from .ruby-version.
pub const LYNX_CACHE_RUBY_STATE: &str = "LYNX_CACHE_RUBY_STATE";

/// JSON-serialized Go version cache. Set by the golang plugin from go.mod.
pub const LYNX_CACHE_GOLANG_STATE: &str = "LYNX_CACHE_GOLANG_STATE";

/// JSON-serialized Rust toolchain cache. Set by the rust-ver plugin from rust-toolchain.toml.
pub const LYNX_CACHE_RUST_STATE: &str = "LYNX_CACHE_RUST_STATE";

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

/// Set to `"1"` to suppress plugin loading failures and start in degraded mode.
pub const LYNX_SAFE_MODE: &str = "LYNX_SAFE_MODE";

/// Set to `"1"` by `lx benchmark` to gate benchmark-only codepaths.
pub const LYNX_BENCHMARK_MODE: &str = "LYNX_BENCHMARK_MODE";

/// Exit code of the last shell command. Exported by the precmd hook before `lx prompt render`.
pub const LYNX_LAST_EXIT_CODE: &str = "LYNX_LAST_EXIT_CODE";

/// Number of background jobs in the current shell. Exported by the precmd hook.
pub const LYNX_BG_JOBS: &str = "LYNX_BG_JOBS";

/// Current vi-mode indicator (e.g. `"insert"`, `"normal"`). Set by the vi-mode plugin.
pub const LYNX_VI_MODE: &str = "LYNX_VI_MODE";

/// Root user marker exported by `lx refresh-state` for prompt symbol logic.
pub const LYNX_USER_IS_ROOT: &str = "LYNX_USER_IS_ROOT";

/// Shell history line number exported by shell hooks.
pub const LYNX_HIST_NUMBER: &str = "LYNX_HIST_NUMBER";

/// Current UNIX timestamp in seconds for time-based prompt segments.
pub const LYNX_NOW_SECS: &str = "LYNX_NOW_SECS";

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

/// System `$PATH` — used for binary discovery (`find_binary`).
pub const PATH: &str = "PATH";

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

/// Returns the canonical plugin state-cache variable name for a plugin.
///
/// Pattern: `LYNX_CACHE_{NAME_UPPERCASE_UNDERSCORED}_STATE`
///
/// Used by community plugin `state.gather` output checks and refresh-state tests.
pub fn cache_state_var(plugin_name: &str) -> String {
    format!(
        "LYNX_CACHE_{}_STATE",
        plugin_name.to_uppercase().replace('-', "_")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_guard_var_normalizes_hyphens() {
        assert_eq!(
            plugin_guard_var("my-plugin"),
            "LYNX_PLUGIN_MY_PLUGIN_LOADED"
        );
    }

    #[test]
    fn plugin_guard_var_uppercase() {
        assert_eq!(plugin_guard_var("git"), "LYNX_PLUGIN_GIT_LOADED");
    }

    #[test]
    fn cache_state_var_normalizes_hyphens() {
        assert_eq!(cache_state_var("my-plugin"), "LYNX_CACHE_MY_PLUGIN_STATE");
    }

    #[test]
    fn cache_state_var_uppercase() {
        assert_eq!(cache_state_var("git"), "LYNX_CACHE_GIT_STATE");
    }
}
