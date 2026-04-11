use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct GitAheadBehindConfig {
    /// Symbol for ahead count. Default: "↑".
    ahead_symbol: Option<String>,
    /// Symbol for behind count. Default: "↓".
    behind_symbol: Option<String>,
}

pub struct GitAheadBehindSegment;

fn git_state_obj(ctx: &RenderContext) -> Option<&serde_json::Map<String, serde_json::Value>> {
    match ctx.cache.get(crate::cache_keys::GIT_STATE)? {
        serde_json::Value::Object(m) => Some(m),
        _ => None,
    }
}

impl Segment for GitAheadBehindSegment {
    fn name(&self) -> &'static str {
        "git_ahead_behind"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GIT_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GitAheadBehindConfig = config.clone().try_into().unwrap_or_default();
        let obj = git_state_obj(ctx)?;

        let ahead = obj.get("ahead").and_then(|v| v.as_u64()).unwrap_or(0);
        let behind = obj.get("behind").and_then(|v| v.as_u64()).unwrap_or(0);

        if ahead == 0 && behind == 0 {
            return None;
        }

        let ahead_sym = cfg.ahead_symbol.unwrap_or_else(|| "↑".to_string());
        let behind_sym = cfg.behind_symbol.unwrap_or_else(|| "↓".to_string());

        let mut parts: Vec<String> = Vec::new();
        if ahead > 0 {
            parts.push(format!("{ahead_sym}{ahead}"));
        }
        if behind > 0 {
            parts.push(format!("{behind_sym}{behind}"));
        }

        Some(RenderedSegment::new(parts.join(" ")).with_cache_key("git_ahead_behind"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_counts(ahead: u64, behind: u64) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::GIT_STATE.into(),
            serde_json::json!({ "branch": "main", "ahead": ahead, "behind": behind }),
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
    fn hidden_when_both_zero() {
        let ctx = ctx_with_counts(0, 0);
        let r = GitAheadBehindSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_ahead_only() {
        let ctx = ctx_with_counts(2, 0);
        let r = GitAheadBehindSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "↑2");
    }

    #[test]
    fn shows_behind_only() {
        let ctx = ctx_with_counts(0, 3);
        let r = GitAheadBehindSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "↓3");
    }

    #[test]
    fn shows_both() {
        let ctx = ctx_with_counts(1, 2);
        let r = GitAheadBehindSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "↑1 ↓2");
    }

    #[test]
    fn custom_symbols() {
        let cfg: toml::Value = toml::from_str(r#"ahead_symbol = "⇡"
behind_symbol = "⇣""#).unwrap();
        let ctx = ctx_with_counts(3, 1);
        let r = GitAheadBehindSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "⇡3 ⇣1");
    }
}
