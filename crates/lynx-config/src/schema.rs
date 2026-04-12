use lynx_core::types::Context;
use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Top-level user configuration, stored at `~/.config/lynx/config.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LynxConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_theme")]
    pub active_theme: String,
    #[serde(default)]
    pub active_context: Context,
    #[serde(default)]
    pub enabled_plugins: Vec<String>,
    #[serde(default)]
    pub sync: SyncConfig,
    /// Intro display settings. Disabled by default.
    #[serde(default)]
    pub intro: IntroConfig,
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

fn default_theme() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    pub remote: Option<String>,
}

/// Intro display configuration, stored under `[intro]` in `config.toml`.
/// Disabled by default — user must opt in via `lx intro on`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntroConfig {
    /// Whether the intro is enabled. Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Slug of the active intro. None = no intro selected.
    #[serde(default)]
    pub active: Option<String>,
}

impl Default for IntroConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            active: None,
        }
    }
}
