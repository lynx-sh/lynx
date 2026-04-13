use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct ViModeConfig {
    /// Label shown in INSERT mode. Default: "INSERT".
    insert_label: Option<String>,
    /// Label shown in NORMAL mode. Default: "NORMAL".
    normal_label: Option<String>,
}

pub struct ViModeSegment;

impl Segment for ViModeSegment {
    fn name(&self) -> &'static str {
        "vi_mode"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: ViModeConfig = config.clone().try_into().unwrap_or_default();
        let mode = ctx.env.get("LYNX_VI_MODE")?;
        let text = match mode.as_str() {
            "insert" => cfg.insert_label.unwrap_or_else(|| "INSERT".to_string()),
            "normal" => cfg.normal_label.unwrap_or_else(|| "NORMAL".to_string()),
            other => other.to_uppercase(),
        };
        Some(RenderedSegment::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_env(pairs: &[(&str, &str)]) -> RenderContext {
        let env = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env,
        }
    }

    #[test]
    fn hidden_when_env_missing() {
        let ctx = ctx_with_env(&[]);
        let r = ViModeSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_insert_label() {
        let ctx = ctx_with_env(&[("LYNX_VI_MODE", "insert")]);
        let r = ViModeSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "INSERT");
    }

    #[test]
    fn shows_normal_label() {
        let ctx = ctx_with_env(&[("LYNX_VI_MODE", "normal")]);
        let r = ViModeSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "NORMAL");
    }

    #[test]
    fn custom_labels() {
        let cfg: toml::Value = toml::from_str(
            r#"
insert_label = "I"
normal_label = "N"
"#,
        )
        .unwrap();
        let ctx = ctx_with_env(&[("LYNX_VI_MODE", "insert")]);
        let r = ViModeSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "I");
    }

    #[test]
    fn unknown_mode_uppercased() {
        let ctx = ctx_with_env(&[("LYNX_VI_MODE", "visual")]);
        let r = ViModeSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "VISUAL");
    }
}
