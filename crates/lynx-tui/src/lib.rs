//! Interactive terminal UI components for Lynx CLI.
//!
//! Provides a reusable `InteractiveList` component (ratatui + crossterm) that
//! any `lx` command can use for browsable, searchable, themed output.
//!
//! Architecture (D-040):
//! - Commands implement `ListItem` for their data type
//! - Call `show()` with items + theme colors
//! - TTY → interactive TUI; pipe/agent → plain text fallback
//!
//! Theme integration (D-041):
//! - TUI chrome uses 5 semantic base colors from the active theme
//! - `TuiColors` carries accent/success/warning/error/muted
//! - Falls back to Tokyo Night defaults when colors are missing

pub(crate) mod gate;
mod item;
mod list;
pub mod onboard;
mod preview;
pub(crate) mod terminal;
pub mod workflow;

pub use item::{ListAction, ListItem, TuiColors};
pub use list::{print_plain, print_plain_multi, run, run_multi, show, show_multi, ListResult};

/// Returns `true` if interactive TUI mode is currently active.
///
/// Checks TTY state, agent environment variables, `LYNX_NO_TUI`, and the
/// config `[tui] enabled` flag (best-effort, falls back to `true`).
///
/// Use this to gate any non-list TUI output (e.g. auto-launching a wizard).
pub fn is_tui_active() -> bool {
    let config_enabled = lynx_config::load().ok().map(|c| c.tui.enabled);
    gate::tui_enabled(config_enabled)
}

/// Default palette colors (Tokyo Night) — used when theme has no [colors] table.
pub mod defaults {
    pub const ACCENT: &str = "#7aa2f7";
    pub const SUCCESS: &str = "#9ece6a";
    pub const WARNING: &str = "#e0af68";
    pub const ERROR: &str = "#f7768e";
    pub const MUTED: &str = "#565f89";
}
