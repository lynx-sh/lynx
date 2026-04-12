use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct PromptCharConfig {
    /// Symbol shown when last exit code was 0. Default: "❯".
    symbol: Option<String>,
    /// Symbol shown when last exit code was non-zero. Default: "❯" (colored red via theme).
    error_symbol: Option<String>,
    /// Symbol shown when user is root (LYNX_USER_IS_ROOT=1). Default: None (falls back to symbol).
    root_symbol: Option<String>,
    /// Symbol shown when inside a git repo. Default: None (falls back to symbol).
    in_git_repo_symbol: Option<String>,
}

pub struct PromptCharSegment;

impl Segment for PromptCharSegment {
    fn name(&self) -> &'static str {
        "prompt_char"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: PromptCharConfig = config.clone().try_into().unwrap_or_default();
        let default_symbol = || "❯".to_string();

        let exit_code: i32 = ctx
            .env
            .get("LYNX_LAST_EXIT_CODE")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let is_error = exit_code != 0;
        let is_root = ctx.env.get("LYNX_USER_IS_ROOT").map(|v| v == "1").unwrap_or(false);
        let in_git_repo = ctx
            .cache
            .get(crate::cache_keys::GIT_STATE)
            .and_then(|v| v.as_object())
            .and_then(|o| o.get("branch"))
            .and_then(|b| b.as_str())
            .map(|b| !b.is_empty())
            .unwrap_or(false);

        // Priority: error > root > git_repo > default
        let symbol = if is_error {
            cfg.error_symbol
                .or(cfg.symbol.clone())
                .unwrap_or_else(default_symbol)
        } else if is_root {
            cfg.root_symbol
                .or(cfg.symbol.clone())
                .unwrap_or_else(default_symbol)
        } else if in_git_repo {
            cfg.in_git_repo_symbol
                .or(cfg.symbol.clone())
                .unwrap_or_else(default_symbol)
        } else {
            cfg.symbol.unwrap_or_else(default_symbol)
        };

        Some(RenderedSegment::new(symbol).with_cache_key("prompt_char"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use serde_json::json;
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
    fn root_symbol_when_root() {
        let cfg: toml::Value = toml::from_str("root_symbol = \"#\"").unwrap();
        let ctx = ctx_with_env(&[("LYNX_USER_IS_ROOT", "1")]);
        let r = PromptCharSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "#");
    }

    #[test]
    fn git_repo_symbol_when_in_repo() {
        let cfg: toml::Value = toml::from_str(r##"in_git_repo_symbol = "±""##).unwrap();
        let mut ctx = ctx_with_env(&[("LYNX_LAST_EXIT_CODE", "0")]);
        ctx.cache.insert(
            crate::cache_keys::GIT_STATE.to_string(),
            serde_json::json!({"branch": "main"}),
        );
        let r = PromptCharSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "±");
    }

    #[test]
    fn error_takes_priority_over_root() {
        let cfg: toml::Value = toml::from_str("error_symbol = \"✗\"\nroot_symbol = \"#\"").unwrap();
        let ctx = ctx_with_env(&[("LYNX_LAST_EXIT_CODE", "1"), ("LYNX_USER_IS_ROOT", "1")]);
        let r = PromptCharSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "✗");
    }

    #[test]
    fn root_takes_priority_over_git_repo() {
        let cfg: toml::Value = toml::from_str("root_symbol = \"#\"\nin_git_repo_symbol = \"±\"").unwrap();
        let mut ctx = ctx_with_env(&[("LYNX_USER_IS_ROOT", "1")]);
        ctx.cache.insert(
            crate::cache_keys::GIT_STATE.to_string(),
            serde_json::json!({"branch": "main"}),
        );
        let r = PromptCharSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "#");
    }

    #[test]
    fn cache_key_is_prompt_char() {
        let ctx = ctx_with_env(&[]);
        let r = PromptCharSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.cache_key.as_deref(), Some("prompt_char"));
    }
}
