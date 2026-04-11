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
    /// Currently active profile name (None = no profile active).
    #[serde(default)]
    pub active_profile: Option<String>,
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
