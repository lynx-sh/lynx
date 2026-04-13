use crate::schema::LynxConfig;
use lynx_core::error::{LynxError, Result};

/// Validate a config before writing it to disk (D-007).
///
/// Called by `save_to` — not called separately in normal usage.
/// Returns a descriptive error with a Fix hint if validation fails.
pub fn validate_before_apply(config: &LynxConfig) -> Result<()> {
    if config.active_theme.is_empty() {
        return Err(LynxError::Config(
            "active_theme must not be empty — set a theme with `lx theme set <name>`".into(),
        ));
    }

    // Reject theme names that could be path traversal attacks.
    if config.active_theme.contains('/') || config.active_theme.contains("..") {
        return Err(LynxError::Config(format!(
            "active_theme '{}' contains invalid characters — use `lx theme list` to see valid names",
            config.active_theme
        )));
    }

    // Plugin names must be non-empty and valid identifiers.
    for plugin in &config.enabled_plugins {
        if plugin.is_empty() {
            return Err(LynxError::Config(
                "enabled_plugins contains an empty name — remove it with `lx plugin list`".into(),
            ));
        }
        if plugin.contains('/') || plugin.contains("..") {
            return Err(LynxError::Config(format!(
                "plugin name '{plugin}' contains invalid characters — use `lx plugin list` to inspect"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_core::types::Context;

    fn base() -> LynxConfig {
        LynxConfig {
            active_theme: "default".into(),
            active_context: Context::Interactive,
            enabled_plugins: vec![],
            ..Default::default()
        }
    }

    #[test]
    fn valid_config_passes() {
        assert!(validate_before_apply(&base()).is_ok());
    }

    #[test]
    fn empty_theme_fails_with_hint() {
        let cfg = LynxConfig {
            active_theme: "".into(),
            ..base()
        };
        let err = validate_before_apply(&cfg).unwrap_err().to_string();
        assert!(err.contains("active_theme"), "{err}");
        assert!(err.contains("lx theme"), "{err}");
    }

    #[test]
    fn path_traversal_theme_rejected() {
        let cfg = LynxConfig {
            active_theme: "../etc/passwd".into(),
            ..base()
        };
        assert!(validate_before_apply(&cfg).is_err());
    }

    #[test]
    fn empty_plugin_name_rejected() {
        let cfg = LynxConfig {
            enabled_plugins: vec!["".into()],
            ..base()
        };
        assert!(validate_before_apply(&cfg).is_err());
    }

    #[test]
    fn plugin_path_traversal_rejected() {
        let cfg = LynxConfig {
            enabled_plugins: vec!["../evil".into()],
            ..base()
        };
        assert!(validate_before_apply(&cfg).is_err());
    }
}
