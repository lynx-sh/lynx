use lynx_core::types::Context;
use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// Context in which a user alias is active.
/// Defaults to Interactive — enforcing D-010/D-004 at the data layer.
/// Aliases tagged `All` load in every non-agent, non-minimal context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AliasContext {
    /// Only loaded in the interactive context (default, safest).
    #[default]
    Interactive,
    /// Loaded in all contexts except agent and minimal.
    All,
}

/// A user-defined alias stored in config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserAlias {
    /// The alias name (e.g. `gs`).
    pub name: String,
    /// The command the alias expands to (e.g. `git status`).
    pub command: String,
    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Which context(s) this alias loads in. Defaults to Interactive.
    #[serde(default)]
    pub context: AliasContext,
}

/// A user-defined PATH entry stored in config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPath {
    /// The filesystem path to prepend to PATH.
    pub path: String,
    /// Optional human-readable label (e.g. "Homebrew sbin").
    #[serde(default)]
    pub label: Option<String>,
}

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
    /// User-defined aliases, stored under `[[aliases]]` in `config.toml`.
    #[serde(default)]
    pub aliases: Vec<UserAlias>,
    /// User-defined PATH entries, stored under `[[paths]]` in `config.toml`.
    #[serde(default)]
    pub paths: Vec<UserPath>,
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

    #[test]
    fn default_config_has_empty_aliases_and_paths() {
        let cfg = LynxConfig::default();
        assert!(cfg.aliases.is_empty());
        assert!(cfg.paths.is_empty());
    }

    #[test]
    fn alias_context_defaults_to_interactive() {
        let ctx = AliasContext::default();
        assert_eq!(ctx, AliasContext::Interactive);
    }

    #[test]
    fn user_alias_deserializes_with_defaults() {
        let toml = r#"
            [[aliases]]
            name = "gs"
            command = "git status"
        "#;
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.aliases.len(), 1);
        let alias = &cfg.aliases[0];
        assert_eq!(alias.name, "gs");
        assert_eq!(alias.command, "git status");
        assert!(alias.description.is_none());
        assert_eq!(alias.context, AliasContext::Interactive);
    }

    #[test]
    fn user_alias_context_all_deserializes() {
        let toml = r#"
            [[aliases]]
            name = "ll"
            command = "ls -la"
            context = "all"
        "#;
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.aliases[0].context, AliasContext::All);
    }

    #[test]
    fn user_path_deserializes_with_defaults() {
        let toml = r#"
            [[paths]]
            path = "/usr/local/sbin"
        "#;
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.paths.len(), 1);
        assert_eq!(cfg.paths[0].path, "/usr/local/sbin");
        assert!(cfg.paths[0].label.is_none());
    }

    #[test]
    fn user_path_with_label_deserializes() {
        let toml = r#"
            [[paths]]
            path = "/opt/homebrew/bin"
            label = "Homebrew"
        "#;
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.paths[0].label.as_deref(), Some("Homebrew"));
    }

    #[test]
    fn existing_config_without_aliases_gets_empty_default() {
        let toml = "active_theme = \"nord\"\n";
        let cfg: LynxConfig = toml::from_str(toml).unwrap();
        assert!(cfg.aliases.is_empty());
        assert!(cfg.paths.is_empty());
    }

    #[test]
    fn aliases_and_paths_roundtrip() {
        let cfg = LynxConfig {
            aliases: vec![UserAlias {
                name: "gs".into(),
                command: "git status".into(),
                description: Some("quick git status".into()),
                context: AliasContext::Interactive,
            }],
            paths: vec![UserPath {
                path: "/usr/local/sbin".into(),
                label: Some("sbin".into()),
            }],
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let back: LynxConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(cfg, back);
    }
}
