use lynx_theme::schema::SegmentConfig;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the active profile name in the prompt.
/// Hidden when no profile is active or when running in agent/minimal context.
/// Add to your theme layout with segment name "profile_badge".
pub struct ProfileBadgeSegment;

impl Segment for ProfileBadgeSegment {
    fn name(&self) -> &'static str {
        "profile_badge"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::PROFILE_STATE)
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        use lynx_core::types::Context;

        // Only show in interactive context — not useful in agent/minimal.
        if ctx.shell_context != Context::Interactive {
            return None;
        }

        let profile_name = ctx
            .cache
            .get(crate::cache_keys::PROFILE_STATE)
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())?;

        if profile_name.is_empty() {
            return None;
        }

        let icon = config.icon.as_deref().unwrap_or("⬡ ");
        Some(RenderedSegment::new(format!("{icon}{profile_name}")).with_cache_key(crate::cache_keys::PROFILE_STATE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ctx_with_profile(name: &str, shell_ctx: lynx_core::types::Context) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::PROFILE_STATE.to_string(),
            serde_json::json!({ "name": name }),
        );
        RenderContext {
            cwd: "/tmp".to_string(),
            shell_context: shell_ctx,
            last_cmd_ms: None,
            cache,
        }
    }

    fn empty_ctx() -> RenderContext {
        RenderContext {
            cwd: "/tmp".to_string(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
        }
    }

    #[test]
    fn hidden_without_cache() {
        let r = ProfileBadgeSegment.render(&Default::default(), &empty_ctx());
        assert!(r.is_none());
    }

    #[test]
    fn shows_profile_name_in_interactive() {
        let r = ProfileBadgeSegment.render(
            &Default::default(),
            &ctx_with_profile("work", lynx_core::types::Context::Interactive),
        );
        assert!(r.is_some());
        assert!(r.unwrap().text.contains("work"));
    }

    #[test]
    fn hidden_in_agent_context() {
        let r = ProfileBadgeSegment.render(
            &Default::default(),
            &ctx_with_profile("work", lynx_core::types::Context::Agent),
        );
        assert!(r.is_none());
    }

    #[test]
    fn hidden_in_minimal_context() {
        let r = ProfileBadgeSegment.render(
            &Default::default(),
            &ctx_with_profile("work", lynx_core::types::Context::Minimal),
        );
        assert!(r.is_none());
    }
}
