use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the current shell name. Lynx is zsh-only, so this always renders "zsh"
/// unless overridden via config.
///
/// TOML config:
/// ```toml
/// [segment.shell]
/// color = { fg = "#0077c2" }
/// # icon = "\uf120"  # terminal icon
/// # name = "zsh"     # override shell name
/// ```
pub struct ShellSegment;

#[derive(Deserialize, Default)]
struct ShellConfig {
    /// Override the shell name. Default: "zsh".
    name: Option<String>,
    /// Icon prepended to the shell name. Default: none.
    icon: Option<String>,
}

impl Segment for ShellSegment {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn render(&self, config: &toml::Value, _ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: ShellConfig = config.clone().try_into().unwrap_or_default();
        let shell_name = cfg.name.unwrap_or_else(|| "zsh".to_string());
        let text = match cfg.icon {
            Some(icon) => format!("{icon} {shell_name}"),
            None => shell_name,
        };
        Some(RenderedSegment::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx() -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[test]
    fn default_is_zsh() {
        let r = ShellSegment.render(&empty_config(), &ctx()).unwrap();
        assert_eq!(r.text, "zsh");
    }

    #[test]
    fn custom_name() {
        let cfg: toml::Value = toml::from_str(r#"name = "fish""#).unwrap();
        let r = ShellSegment.render(&cfg, &ctx()).unwrap();
        assert_eq!(r.text, "fish");
    }

    #[test]
    fn icon_prepended() {
        let cfg: toml::Value = toml::from_str(r#"icon = ">""#).unwrap();
        let r = ShellSegment.render(&cfg, &ctx()).unwrap();
        assert_eq!(r.text, "> zsh");
    }
}
