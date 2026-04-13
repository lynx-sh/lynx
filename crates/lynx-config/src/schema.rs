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
    /// TUI display settings.
    #[serde(default)]
    pub tui: TuiConfig,
    /// Set to true after the user completes `lx onboard`.
    /// Prevents `lx setup` from re-launching the wizard on subsequent runs.
    #[serde(default)]
    pub onboarding_complete: bool,
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

fn default_theme() -> String {
    "default".to_string()
}

/// TUI display configuration, stored under `[tui]` in `config.toml`.
/// Enabled by default — set `enabled = false` to always use plain terminal output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TuiConfig {
    /// Whether to use interactive TUI mode. Default: true.
    /// When false, all commands fall back to plain text output.
    #[serde(default = "default_tui_enabled")]
    pub enabled: bool,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn default_tui_enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    pub remote: Option<String>,
}

/// Intro display configuration, stored under `[intro]` in `config.toml`.
/// Disabled by default — user must opt in via `lx intro on`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct IntroConfig {
    /// Whether the intro is enabled. Default: false.
    #[serde(default)]
    pub enabled: bool,
    /// Slug of the active intro. None = no intro selected.
    #[serde(default)]
    pub active: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = LynxConfig::default();
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(cfg.active_theme, "default");
        assert_eq!(cfg.active_context, Context::Interactive);
        assert!(cfg.enabled_plugins.is_empty());
        assert!(cfg.sync.remote.is_none());
        assert!(!cfg.intro.enabled);
        assert!(cfg.intro.active.is_none());
        assert!(cfg.tui.enabled);
        assert!(!cfg.onboarding_complete);
    }

    #[test]
    fn config_deserialize_minimal() {
        let toml = "";
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.active_theme, "default");
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn config_deserialize_full() {
        let toml = r#"
            schema_version = 1
            active_theme = "nord"
            active_context = "agent"
            enabled_plugins = ["git", "fzf"]

            [sync]
            remote = "git@github.com:user/dotfiles.git"

            [intro]
            enabled = true
            active = "default"
        "#;
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.active_theme, "nord");
        assert_eq!(cfg.active_context, Context::Agent);
        assert_eq!(cfg.enabled_plugins, vec!["git", "fzf"]);
        assert_eq!(
            cfg.sync.remote.as_deref(),
            Some("git@github.com:user/dotfiles.git")
        );
        assert!(cfg.intro.enabled);
        assert_eq!(cfg.intro.active.as_deref(), Some("default"));
    }

    #[test]
    fn config_serialize_roundtrip() {
        let cfg = LynxConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let back: LynxConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn intro_config_default() {
        let ic = IntroConfig::default();
        assert!(!ic.enabled);
        assert!(ic.active.is_none());
    }

    #[test]
    fn sync_config_default() {
        let sc = SyncConfig::default();
        assert!(sc.remote.is_none());
    }

    #[test]
    fn tui_config_default_enabled() {
        let tc = TuiConfig::default();
        assert!(tc.enabled);
    }

    #[test]
    fn tui_config_disabled_deserializes() {
        let toml = "[tui]\nenabled = false\n";
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert!(!cfg.tui.enabled);
    }

    #[test]
    fn existing_config_without_tui_gets_enabled_default() {
        // Backward compat: configs written before [tui] existed must default to enabled.
        let toml = "active_theme = \"nord\"\n";
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert!(cfg.tui.enabled);
        assert!(!cfg.onboarding_complete);
    }

    #[test]
    fn onboarding_complete_roundtrip() {
        let toml = "onboarding_complete = true\n";
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert!(cfg.onboarding_complete);
    }
}
