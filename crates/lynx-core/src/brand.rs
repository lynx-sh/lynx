//! Single source of truth for all brand/naming constants.
//! To rename the framework: edit ONLY this file.

// ── Identity ─────────────────────────────────────────────────────────────────

pub const NAME: &str = "Lynx";
pub const CLI: &str = "lx";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "https://github.com/lynx-sh/lynx";

// ── Directory layout (relative to $HOME) ─────────────────────────────────────

/// Base config/install directory: `~/.config/lynx`
pub const CONFIG_DIR: &str = ".config/lynx";

// ── File names ────────────────────────────────────────────────────────────────

pub const CONFIG_FILE: &str = "config.toml";
pub const TASKS_FILE: &str = "tasks.toml";
pub const PLUGIN_MANIFEST: &str = "plugin.toml";
pub const THEME_EXT: &str = ".toml";

// ── Daemon ────────────────────────────────────────────────────────────────────

pub const DAEMON_NAME: &str = "lynx-daemon";

/// macOS launchd agent label — must match the plist `Label` key exactly.
pub const LAUNCHD_LABEL: &str = "com.lynx.daemon";

/// Linux systemd user service name.
pub const SYSTEMD_SERVICE: &str = "lynx-daemon.service";

// ── Defaults ─────────────────────────────────────────────────────────────────

pub const DEFAULT_THEME: &str = "default";

// ── Shell integration ─────────────────────────────────────────────────────────

/// Taps configuration file name.
pub const TAPS_FILE: &str = "taps.toml";
/// Official registry index URL.
pub const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/lynx-sh/registry/main/index.toml";

/// The line written to `.zshrc` by `lx setup` and matched by `lx uninstall`.
/// Must be a single exact string — both install and uninstall use this for matching.
pub const ZSHRC_INIT_LINE: &str =
    r#"source "${HOME}/.config/lynx/shell/init.zsh""#;
