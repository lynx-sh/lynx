use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct HistNumberConfig {
    /// Prefix before the history number. Default: empty.
    prefix: Option<String>,
}

pub struct HistNumberSegment;

impl Segment for HistNumberSegment {
    fn name(&self) -> &'static str {
        "hist_number"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: HistNumberConfig = config.clone().try_into().unwrap_or_default();

        let num = ctx.env.get(lynx_core::env_vars::LYNX_HIST_NUMBER)?;
        if num.is_empty() {
            return None;
        }

        let prefix = cfg.prefix.as_deref().unwrap_or("");
        let text = format!("{prefix}{num}");
        Some(RenderedSegment::new(&text).with_cache_key("hist_number"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_hist(num: &str) -> RenderContext {
        let mut env = HashMap::new();
        env.insert(
            lynx_core::env_vars::LYNX_HIST_NUMBER.to_string(),
            num.to_string(),
        );
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env,
        }
    }

    #[test]
    fn hidden_when_absent() {
        let ctx = RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        };
        assert!(HistNumberSegment.render(&empty_config(), &ctx).is_none());
    }

    #[test]
    fn hidden_when_empty() {
        let ctx = ctx_with_hist("");
        assert!(HistNumberSegment.render(&empty_config(), &ctx).is_none());
    }

    #[test]
    fn shows_number() {
        let ctx = ctx_with_hist("42");
        let seg = HistNumberSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(seg.text, "42");
    }

    #[test]
    fn custom_prefix() {
        let cfg: toml::Value = toml::from_str(r#"prefix = "!""#).unwrap();
        let ctx = ctx_with_hist("100");
        let seg = HistNumberSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(seg.text, "!100");
    }

    #[test]
    fn cache_key_is_hist_number() {
        let ctx = ctx_with_hist("1");
        let seg = HistNumberSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(seg.cache_key.as_deref(), Some("hist_number"));
    }
}
