use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct GitStashConfig {
    /// Symbol prepended to the stash count. Default: "⚑".
    symbol: Option<String>,
}

pub struct GitStashSegment;

use super::git_common::git_state_obj;

impl Segment for GitStashSegment {
    fn name(&self) -> &'static str {
        "git_stash"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GIT_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GitStashConfig = config.clone().try_into().unwrap_or_default();
        let obj = git_state_obj(ctx)?;
        let stash = obj.get("stash").and_then(|v| v.as_u64()).unwrap_or(0);
        if stash == 0 {
            return None;
        }
        let symbol = cfg.symbol.unwrap_or_else(|| "⚑".to_string());
        Some(RenderedSegment::new(format!("{symbol} {stash}")).with_cache_key("git_stash"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_stash(count: u64) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::GIT_STATE.into(),
            serde_json::json!({ "branch": "main", "stash": count }),
        );
        RenderContext {
            cwd: "/repo".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
            env: HashMap::new(),
        }
    }

    #[test]
    fn hidden_when_stash_zero() {
        let ctx = ctx_with_stash(0);
        let r = GitStashSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_no_cache() {
        let ctx = RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        };
        let r = GitStashSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_stash_count() {
        let ctx = ctx_with_stash(3);
        let r = GitStashSegment.render(&empty_config(), &ctx).unwrap();
        assert!(r.text.contains('3'), "expected count: {}", r.text);
        assert!(r.text.contains('⚑'), "expected default symbol: {}", r.text);
    }

    #[test]
    fn custom_symbol() {
        let cfg: toml::Value = toml::from_str(r#"symbol = "S""#).unwrap();
        let ctx = ctx_with_stash(2);
        let r = GitStashSegment.render(&cfg, &ctx).unwrap();
        assert!(
            r.text.starts_with('S'),
            "expected custom symbol: {}",
            r.text
        );
    }
}
