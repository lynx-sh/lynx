use std::collections::HashMap;

use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct ContextBadgeConfig {
    label: Option<HashMap<String, String>>,
}

pub struct ContextBadgeSegment;

impl Segment for ContextBadgeSegment {
    fn name(&self) -> &'static str {
        "context_badge"
    }

    /// Hide in interactive by default — show only in agent/minimal unless
    /// the theme explicitly configures show_in/hide_in.
    fn default_hide_in(&self) -> &[&str] {
        &["interactive"]
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        use lynx_core::types::Context;

        let cfg: ContextBadgeConfig = config.clone().try_into().unwrap_or_default();

        let ctx_key = match ctx.shell_context {
            Context::Agent => "agent",
            Context::Minimal => "minimal",
            Context::Interactive => "interactive",
        };

        let text = cfg
            .label
            .as_ref()
            .and_then(|m| m.get(ctx_key).cloned())
            .unwrap_or_else(|| match ctx.shell_context {
                Context::Agent => "AI".to_string(),
                Context::Minimal => "MIN".to_string(),
                Context::Interactive => "INT".to_string(),
            });

        Some(RenderedSegment::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use lynx_core::types::Context;

    fn ctx(shell_context: Context) -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context,
            last_cmd_ms: None,
            cache: std::collections::HashMap::new(),
            env: std::collections::HashMap::new(),
        }
    }

    // Note: show_in/hide_in visibility is enforced by the evaluator, not the segment.
    // These tests verify render output only.

    #[test]
    fn renders_ai_in_agent_context() {
        let r = ContextBadgeSegment.render(&empty_config(), &ctx(Context::Agent));
        assert_eq!(r.unwrap().text, "AI");
    }

    #[test]
    fn renders_min_in_minimal_context() {
        let r = ContextBadgeSegment.render(&empty_config(), &ctx(Context::Minimal));
        assert_eq!(r.unwrap().text, "MIN");
    }

    #[test]
    fn renders_int_in_interactive_context() {
        // render() always produces output — evaluator decides visibility
        let r = ContextBadgeSegment.render(&empty_config(), &ctx(Context::Interactive));
        assert_eq!(r.unwrap().text, "INT");
    }

    #[test]
    fn custom_label_from_config() {
        let cfg: toml::Value = toml::from_str(
            r#"[label]
agent = "🤖"
"#,
        )
        .unwrap();
        let r = ContextBadgeSegment.render(&cfg, &ctx(Context::Agent));
        assert_eq!(r.unwrap().text, "🤖");
    }

    #[test]
    fn default_hide_in_includes_interactive() {
        assert!(ContextBadgeSegment
            .default_hide_in()
            .contains(&"interactive"));
    }
}
