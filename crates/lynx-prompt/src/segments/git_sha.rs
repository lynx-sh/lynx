use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct GitShaConfig {
    /// Number of hex characters to show. Default: 7, max: 40.
    length: Option<usize>,
    /// String prepended to the SHA. Default: empty.
    prefix: Option<String>,
}

pub struct GitShaSegment;

fn git_state_str<'a>(ctx: &'a RenderContext, key: &str) -> Option<&'a str> {
    match ctx.cache.get(crate::cache_keys::GIT_STATE)? {
        serde_json::Value::Object(m) => m.get(key)?.as_str(),
        _ => None,
    }
}

impl Segment for GitShaSegment {
    fn name(&self) -> &'static str {
        "git_sha"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GIT_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GitShaConfig = config.clone().try_into().unwrap_or_default();
        let sha = git_state_str(ctx, "sha")?;
        if sha.is_empty() {
            return None;
        }

        let length = cfg.length.unwrap_or(7).min(sha.len()).min(40);
        let truncated = &sha[..length];
        let prefix = cfg.prefix.as_deref().unwrap_or("");

        let text = format!("{prefix}{truncated}");
        Some(RenderedSegment::new(&text).with_cache_key("git_sha"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_sha(sha: &str) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::GIT_STATE.into(),
            serde_json::json!({"branch": "main", "sha": sha}),
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
    fn hidden_when_no_git_state() {
        let ctx = RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        };
        assert!(GitShaSegment.render(&empty_config(), &ctx).is_none());
    }

    #[test]
    fn hidden_when_sha_empty() {
        let ctx = ctx_with_sha("");
        assert!(GitShaSegment.render(&empty_config(), &ctx).is_none());
    }

    #[test]
    fn default_truncates_to_7() {
        let ctx = ctx_with_sha("abc1234def5678901234567890abcdef12345678");
        let seg = GitShaSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(seg.text, "abc1234");
    }

    #[test]
    fn custom_length() {
        let cfg: toml::Value = toml::from_str("length = 12").unwrap();
        let ctx = ctx_with_sha("abc1234def5678901234567890abcdef12345678");
        let seg = GitShaSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(seg.text, "abc1234def56");
    }

    #[test]
    fn short_sha_not_truncated_below_actual_length() {
        let ctx = ctx_with_sha("abc12");
        let seg = GitShaSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(seg.text, "abc12");
    }

    #[test]
    fn custom_prefix() {
        let cfg: toml::Value = toml::from_str("prefix = \"#\"").unwrap();
        let ctx = ctx_with_sha("abc1234def5678");
        let seg = GitShaSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(seg.text, "#abc1234");
    }

    #[test]
    fn cache_key_is_git_sha() {
        let ctx = ctx_with_sha("abc1234def5678");
        let seg = GitShaSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(seg.cache_key.as_deref(), Some("git_sha"));
    }
}
