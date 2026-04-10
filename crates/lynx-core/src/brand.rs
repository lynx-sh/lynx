//! Single source of truth for all brand/naming constants.
//! To rename the framework: edit ONLY this file.

pub const NAME: &str = "Lynx";
pub const CLI: &str = "lx";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "https://github.com/proxikal/lynx";
pub const CONFIG_DIR: &str = ".config/lynx";
pub const DAEMON_NAME: &str = "lynx-daemon";
pub const PLUGIN_MANIFEST: &str = "plugin.toml";
pub const THEME_EXT: &str = ".toml";
