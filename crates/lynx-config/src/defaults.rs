use crate::schema::{IntroConfig, LynxConfig, SyncConfig, TuiConfig, CURRENT_SCHEMA_VERSION};
use lynx_core::types::Context;

impl Default for LynxConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            active_theme: "default".to_string(),
            active_context: Context::Interactive,
            enabled_plugins: vec![],
            sync: SyncConfig { remote: None },
            intro: IntroConfig::default(),
            tui: TuiConfig::default(),
            onboarding_complete: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_default() {
        let cfg = LynxConfig::default();
        assert_eq!(cfg.active_theme, "default");
    }

    #[test]
    fn default_context_is_interactive() {
        let cfg = LynxConfig::default();
        assert_eq!(cfg.active_context, Context::Interactive);
    }

    #[test]
    fn default_schema_version_matches_current() {
        let cfg = LynxConfig::default();
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn default_plugins_empty() {
        let cfg = LynxConfig::default();
        assert!(cfg.enabled_plugins.is_empty());
    }

    #[test]
    fn default_intro_disabled() {
        let cfg = LynxConfig::default();
        assert!(!cfg.intro.enabled);
    }
}
