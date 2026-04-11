use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the Ruby version pinned by the current project (.ruby-version).
/// Hidden when the ruby plugin is not active or no version file is present.
pub struct RubyVersionSegment;

impl Segment for RubyVersionSegment {
    fn name(&self) -> &'static str {
        "ruby_version"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::RUBY_STATE)
    }

    fn render(&self, _config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let state = ctx.cache.get(crate::cache_keys::RUBY_STATE)?;
        let version = state.get("version")?.as_str()?;
        if version.is_empty() {
            return None;
        }
        Some(
            RenderedSegment::new(format!("💎 {version}"))
                .with_cache_key(crate::cache_keys::RUBY_STATE),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with(version: &str) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::RUBY_STATE.to_string(),
            serde_json::json!({ "version": version }),
        );
        RenderContext {
            cwd: "/tmp".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
            env: HashMap::new(),
        }
    }

    fn empty_ctx() -> RenderContext {
        RenderContext {
            cwd: "/tmp".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[test]
    fn hidden_without_cache() {
        assert!(RubyVersionSegment.render(&empty_config(), &empty_ctx()).is_none());
    }

    #[test]
    fn shows_version() {
        let r = RubyVersionSegment.render(&empty_config(), &ctx_with("3.3.0")).unwrap();
        assert!(r.text.contains("3.3.0"), "text: {}", r.text);
    }

    #[test]
    fn hidden_on_empty_version() {
        assert!(RubyVersionSegment.render(&empty_config(), &ctx_with("")).is_none());
    }
}
