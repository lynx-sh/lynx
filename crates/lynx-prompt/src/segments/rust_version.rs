use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the Rust toolchain channel pinned by the current project (rust-toolchain.toml).
/// Hidden when the rust-ver plugin is not active or no toolchain file is present.
pub struct RustVersionSegment;

impl Segment for RustVersionSegment {
    fn name(&self) -> &'static str {
        "rust_version"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::RUST_STATE)
    }

    fn render(&self, _config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let state = ctx.cache.get(crate::cache_keys::RUST_STATE)?;
        let version = state.get("version")?.as_str()?;
        if version.is_empty() {
            return None;
        }
        Some(
            RenderedSegment::new(format!("🦀 {version}"))
                .with_cache_key(crate::cache_keys::RUST_STATE),
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
            crate::cache_keys::RUST_STATE.to_string(),
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
        assert!(RustVersionSegment.render(&empty_config(), &empty_ctx()).is_none());
    }

    #[test]
    fn shows_channel() {
        let r = RustVersionSegment.render(&empty_config(), &ctx_with("stable")).unwrap();
        assert!(r.text.contains("stable"), "text: {}", r.text);
    }

    #[test]
    fn shows_pinned_version() {
        let r = RustVersionSegment.render(&empty_config(), &ctx_with("1.78.0")).unwrap();
        assert!(r.text.contains("1.78.0"));
    }

    #[test]
    fn hidden_on_empty_version() {
        assert!(RustVersionSegment.render(&empty_config(), &ctx_with("")).is_none());
    }
}
