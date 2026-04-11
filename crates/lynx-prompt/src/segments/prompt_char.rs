use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct PromptCharConfig {
    /// Symbol shown when last exit code was 0. Default: "❯".
    symbol: Option<String>,
    /// Symbol shown when last exit code was non-zero. Default: "❯" (colored red via theme).
    error_symbol: Option<String>,
}

pub struct PromptCharSegment;

impl Segment for PromptCharSegment {
    fn name(&self) -> &'static str {
        "prompt_char"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: PromptCharConfig = config.clone().try_into().unwrap_or_default();
        let exit_code: i32 = ctx
            .env
            .get("LYNX_LAST_EXIT_CODE")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let is_error = exit_code != 0;

        let symbol = if is_error {
            cfg.error_symbol
                .or(cfg.symbol)
                .unwrap_or_else(|| "❯".to_string())
        } else {
            cfg.symbol.unwrap_or_else(|| "❯".to_string())
        };

        // Emit the symbol with an error tag so the renderer (or theme color) can color it.
        // We use cache_key to allow theme color lookup by segment name.
        let mut seg = RenderedSegment::new(symbol);
        // Tag with segment name so theme can apply `[segment.prompt_char] color` config.
        seg.cache_key = Some("prompt_char".to_string());
        Some(seg)
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
    fn shows_default_symbol_on_success() {
        let ctx = ctx_with_env(&[("LYNX_LAST_EXIT_CODE", "0")]);
        let r = PromptCharSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "❯");
    }

    #[test]
    fn shows_default_symbol_when_no_exit_code() {
        let ctx = ctx_with_env(&[]);
        let r = PromptCharSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "❯");
    }

    #[test]
    fn uses_error_symbol_on_nonzero_exit() {
        let cfg: toml::Value = toml::from_str(r#"
symbol = "❯"
error_symbol = "✗"
"#).unwrap();
        let ctx = ctx_with_env(&[("LYNX_LAST_EXIT_CODE", "1")]);
        let r = PromptCharSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "✗");
    }

    #[test]
    fn falls_back_to_symbol_as_error_symbol() {
        let cfg: toml::Value = toml::from_str(r#"symbol = "→""#).unwrap();
        let ctx = ctx_with_env(&[("LYNX_LAST_EXIT_CODE", "127")]);
        let r = PromptCharSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "→");
    }

    #[test]
    fn cache_key_is_prompt_char() {
        let ctx = ctx_with_env(&[]);
        let r = PromptCharSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.cache_key.as_deref(), Some("prompt_char"));
    }
}
