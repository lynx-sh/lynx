use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct ProfileBadgeConfig {
    icon: Option<String>,
}

/// Shows the active profile name in the prompt.
/// Hidden when no profile is active.
/// Defaults to hiding in agent and minimal contexts.
pub struct ProfileBadgeSegment;

impl Segment for ProfileBadgeSegment {
    fn name(&self) -> &'static str {
        "profile_badge"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::PROFILE_STATE)
    }

    fn default_hide_in(&self) -> &[&str] {
        &["agent", "minimal"]
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: ProfileBadgeConfig = config.clone().try_into().unwrap_or_default();

        let profile_name = ctx
            .cache
            .get(crate::cache_keys::PROFILE_STATE)
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())?;

        if profile_name.is_empty() {
            return None;
        }

        let icon = cfg.icon.as_deref().unwrap_or("⬡ ");
        Some(
            RenderedSegment::new(format!("{icon}{profile_name}"))
                .with_cache_key(crate::cache_keys::PROFILE_STATE),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
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

    // Note: agent/minimal hiding is enforced by the evaluator via default_hide_in().
    // render() here only tests output — not visibility.

    #[test]
    fn hidden_without_cache() {
        let r = ProfileBadgeSegment.render(&empty_config(), &empty_ctx());
        assert!(r.is_none());
    }

    #[test]
    fn shows_profile_name() {
        let r = ProfileBadgeSegment.render(
            &empty_config(),
            &ctx_with_profile("work", lynx_core::types::Context::Interactive),
        );
        assert!(r.is_some());
        assert!(r.unwrap().text.contains("work"));
    }

    #[test]
    fn default_hide_in_excludes_agent_and_minimal() {
        let hide = ProfileBadgeSegment.default_hide_in();
        assert!(hide.contains(&"agent"));
        assert!(hide.contains(&"minimal"));
    }
}
