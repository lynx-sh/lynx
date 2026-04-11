use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the Go version required by the current project (from go.mod).
/// Hidden when the golang plugin is not active or no go.mod is present.
pub struct GolangVersionSegment;

impl Segment for GolangVersionSegment {
    fn name(&self) -> &'static str {
        "golang_version"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GOLANG_STATE)
    }

    fn render(&self, _config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let state = ctx.cache.get(crate::cache_keys::GOLANG_STATE)?;
        let version = state.get("version")?.as_str()?;
        if version.is_empty() {
            return None;
        }
        Some(
            RenderedSegment::new(format!("🐹 {version}"))
                .with_cache_key(crate::cache_keys::GOLANG_STATE),
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
            crate::cache_keys::GOLANG_STATE.to_string(),
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
        assert!(GolangVersionSegment.render(&empty_config(), &empty_ctx()).is_none());
    }

    #[test]
    fn shows_version() {
        let r = GolangVersionSegment.render(&empty_config(), &ctx_with("1.22.3")).unwrap();
        assert!(r.text.contains("1.22.3"), "text: {}", r.text);
    }

    #[test]
    fn hidden_on_empty_version() {
        assert!(GolangVersionSegment.render(&empty_config(), &ctx_with("")).is_none());
    }
}
