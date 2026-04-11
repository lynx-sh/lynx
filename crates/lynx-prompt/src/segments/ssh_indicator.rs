use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct SshIndicatorConfig {
    /// Symbol shown when in an SSH session. Default: "ssh".
    symbol: Option<String>,
}

pub struct SshIndicatorSegment;

impl Segment for SshIndicatorSegment {
    fn name(&self) -> &'static str {
        "ssh_indicator"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: SshIndicatorConfig = config.clone().try_into().unwrap_or_default();
        let is_ssh = ctx.env.contains_key("SSH_CONNECTION") || ctx.env.contains_key("SSH_TTY");
        if !is_ssh {
            return None;
        }
        let symbol = cfg.symbol.unwrap_or_else(|| "ssh".to_string());
        Some(RenderedSegment::new(symbol))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_env(pairs: &[(&str, &str)]) -> RenderContext {
        let env = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env,
        }
    }

    #[test]
    fn hidden_outside_ssh() {
        let ctx = ctx_with_env(&[]);
        let r = SshIndicatorSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_default_symbol_in_ssh() {
        let ctx = ctx_with_env(&[("SSH_CONNECTION", "1.2.3.4 22 5.6.7.8 12345")]);
        let r = SshIndicatorSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "ssh");
    }

    #[test]
    fn shows_custom_symbol() {
        let cfg: toml::Value = toml::from_str(r#"symbol = "🔒""#).unwrap();
        let ctx = ctx_with_env(&[("SSH_TTY", "/dev/pts/0")]);
        let r = SshIndicatorSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "🔒");
    }
}
