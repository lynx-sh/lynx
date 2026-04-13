//! Single source of truth for the TUI activation gate.
//!
//! All interactive TUI components (list, workflow, onboard) call `tui_enabled()`
//! before entering ratatui mode. This prevents any duplicate gate logic and ensures
//! agents, scripts, and CI always receive plain text output.
//!
//! ## Gate checks (all must pass for TUI to activate)
//! 1. stdout is a TTY — piped output always gets plain text
//! 2. `LYNX_CONTEXT` is not `"agent"` — Lynx agent context disables TUI
//! 3. `LYNX_NO_TUI` is not set — explicit user/script opt-out
//! 4. `CLAUDECODE` is not set — Claude Code terminal sessions get plain text
//! 5. `CURSOR_CLI` is not set — Cursor integrated terminal gets plain text
//! 6. `CI` is not `"true"` or `"1"` — CI pipelines get plain text
//! 7. `config_enabled` is not `Some(false)` — user config opt-out

use crossterm::tty::IsTty;
use std::io;
use lynx_core::env_vars;

/// Returns `true` if interactive TUI mode should be used.
///
/// `config_enabled` — pass the value of `config.tui.enabled` if the config is
/// available at the call site, or `None` to skip the config check (e.g. before
/// config is loaded). Passing `Some(false)` disables TUI regardless of other checks.
pub(crate) fn tui_enabled(config_enabled: Option<bool>) -> bool {
    // Config opt-out takes priority — checked first to short-circuit cheaply.
    if config_enabled == Some(false) {
        return false;
    }

    // Non-TTY: piped, redirected, or captured output.
    if !io::stdout().is_tty() {
        return false;
    }

    // Explicit user/script opt-out via env var.
    if std::env::var(env_vars::LYNX_NO_TUI).is_ok() {
        return false;
    }

    // Lynx agent context (set by lx init --context agent).
    if std::env::var(env_vars::LYNX_CONTEXT).as_deref() == Ok("agent") {
        return false;
    }

    // Claude Code spawns terminals with CLAUDECODE set.
    if std::env::var(env_vars::CLAUDECODE).is_ok() {
        return false;
    }

    // Cursor integrated terminal.
    if std::env::var(env_vars::CURSOR_CLI).is_ok() {
        return false;
    }

    // CI pipelines (GitHub Actions, CircleCI, etc.).
    if matches!(
        std::env::var(env_vars::CI).as_deref(),
        Ok("true") | Ok("1")
    ) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_false_disables_tui() {
        // Can test this without touching env or TTY state.
        // TTY check will return false in test environment anyway,
        // but config_enabled = Some(false) is checked first.
        assert!(!tui_enabled(Some(false)));
    }

    #[test]
    fn config_none_does_not_force_disable() {
        // None means "no config opinion" — other checks still apply.
        // In a non-TTY test environment this will still return false,
        // but the function must not panic.
        let _ = tui_enabled(None);
    }

    #[test]
    fn config_true_does_not_force_enable() {
        // Some(true) does not bypass TTY check — all checks must pass.
        // In a non-TTY test runner this should be false.
        assert!(!tui_enabled(Some(true)));
    }
}
