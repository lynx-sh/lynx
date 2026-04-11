use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the Node.js version pinned by the current project (.node-version / .nvmrc).
/// Hidden when the node plugin is not active or no version file is present in the project.
pub struct NodeVersionSegment;

impl Segment for NodeVersionSegment {
    fn name(&self) -> &'static str {
        "node_version"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::NODE_STATE)
    }

    fn render(&self, _config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let state = ctx.cache.get(crate::cache_keys::NODE_STATE)?;
        let version = state.get("version")?.as_str()?;
        if version.is_empty() {
            return None;
        }
        // Strip leading 'v' added by some tools (e.g. `node --version`).
        let ver = version.trim_start_matches('v');
        Some(
            RenderedSegment::new(format!(" {ver}"))
                .with_cache_key(crate::cache_keys::NODE_STATE),
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
            crate::cache_keys::NODE_STATE.to_string(),
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
        assert!(NodeVersionSegment.render(&empty_config(), &empty_ctx()).is_none());
    }

    #[test]
    fn shows_version() {
        let r = NodeVersionSegment.render(&empty_config(), &ctx_with("20.11.0")).unwrap();
        assert!(r.text.contains("20.11.0"), "text: {}", r.text);
    }

    #[test]
    fn strips_leading_v() {
        let r = NodeVersionSegment.render(&empty_config(), &ctx_with("v20.11.0")).unwrap();
        assert!(r.text.contains("20.11.0"));
        assert!(!r.text.contains("vv"));
    }

    #[test]
    fn hidden_on_empty_version() {
        assert!(NodeVersionSegment.render(&empty_config(), &ctx_with("")).is_none());
    }
}
