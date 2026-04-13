use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct ExitCodeConfig {
    /// Symbol prepended to the exit code. Default: "✘".
    symbol: Option<String>,
}

pub struct ExitCodeSegment;

impl Segment for ExitCodeSegment {
    fn name(&self) -> &'static str {
        "exit_code"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: ExitCodeConfig = config.clone().try_into().unwrap_or_default();
        let code_str = ctx.env.get("LYNX_LAST_EXIT_CODE")?;
        let code: i32 = code_str.parse().unwrap_or(0);
        if code == 0 {
            return None;
        }
        let symbol = cfg.symbol.unwrap_or_else(|| "✘".to_string());
        Some(RenderedSegment::new(format!("{symbol} {code}")))
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
    fn hidden_when_exit_code_zero() {
        let ctx = ctx_with_env(&[("LYNX_LAST_EXIT_CODE", "0")]);
        let r = ExitCodeSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_env_missing() {
        let ctx = ctx_with_env(&[]);
        let r = ExitCodeSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_nonzero_exit_code() {
        let ctx = ctx_with_env(&[("LYNX_LAST_EXIT_CODE", "127")]);
        let r = ExitCodeSegment.render(&empty_config(), &ctx).unwrap();
        assert!(r.text.contains("127"), "expected code in output: {}", r.text);
        assert!(r.text.contains('✘'), "expected default symbol: {}", r.text);
    }

    #[test]
    fn custom_symbol() {
        let cfg: toml::Value = toml::from_str(r#"symbol = "ERR""#).unwrap();
        let ctx = ctx_with_env(&[("LYNX_LAST_EXIT_CODE", "1")]);
        let r = ExitCodeSegment.render(&cfg, &ctx).unwrap();
        assert!(r.text.starts_with("ERR"), "expected custom symbol: {}", r.text);
    }
}
