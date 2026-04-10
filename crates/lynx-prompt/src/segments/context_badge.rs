use lynx_core::types::Context;
use lynx_theme::schema::SegmentConfig;

use crate::segment::{RenderContext, RenderedSegment, Segment};

pub struct ContextBadgeSegment;

impl Segment for ContextBadgeSegment {
    fn name(&self) -> &'static str {
        "context_badge"
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        // Only show in agent / minimal contexts — or if show_in is configured.
        let should_show = if let Some(show_in) = &config.show_in {
            let ctx_str = match ctx.shell_context {
                Context::Agent => "agent",
                Context::Minimal => "minimal",
                Context::Interactive => "interactive",
            };
            show_in.iter().any(|s| s == ctx_str)
        } else {
            // Default: show only in non-interactive contexts.
            !matches!(ctx.shell_context, Context::Interactive)
        };

        if !should_show {
            return None;
        }

        let label = config.label.as_ref().and_then(|m| {
            let key = match ctx.shell_context {
                Context::Agent => "agent",
                Context::Minimal => "minimal",
                Context::Interactive => "interactive",
            };
            m.get(key).cloned()
        });

        let text = label.unwrap_or_else(|| match ctx.shell_context {
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
    use std::collections::HashMap;

    fn ctx(shell_context: Context) -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context,
            last_cmd_ms: None,
            cache: HashMap::new(),
        }
    }

    #[test]
    fn hides_in_interactive() {
        let r = ContextBadgeSegment.render(&Default::default(), &ctx(Context::Interactive));
        assert!(r.is_none());
    }

    #[test]
    fn shows_in_agent() {
        let r = ContextBadgeSegment.render(&Default::default(), &ctx(Context::Agent));
        assert!(r.is_some());
        assert_eq!(r.unwrap().text, "AI");
    }

    #[test]
    fn shows_in_minimal() {
        let r = ContextBadgeSegment.render(&Default::default(), &ctx(Context::Minimal));
        assert!(r.is_some());
        assert_eq!(r.unwrap().text, "MIN");
    }

    #[test]
    fn custom_label_from_config() {
        let mut labels = std::collections::HashMap::new();
        labels.insert("agent".to_string(), "🤖".to_string());
        let cfg = SegmentConfig {
            show_in: Some(vec!["agent".into()]),
            label: Some(labels),
            ..Default::default()
        };
        let r = ContextBadgeSegment.render(&cfg, &ctx(Context::Agent));
        assert_eq!(r.unwrap().text, "🤖");
    }
}
