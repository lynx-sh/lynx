use lynx_theme::schema::SegmentConfig;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the current kubectl context and namespace.
/// Hidden when kubectl is not installed or no context is active.
/// Turns red when the context name matches prod_pattern (configurable in theme).
pub struct KubectlContextSegment;

impl Segment for KubectlContextSegment {
    fn name(&self) -> &'static str {
        "kubectl_context"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some("kubectl_state")
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        let state = ctx.cache.get("kubectl_state")?;

        let context = state.get("context")?.as_str()?;
        if context.is_empty() || context == "default" {
            return None;
        }

        let namespace = state
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let is_prod = config
            .prod_pattern
            .as_deref()
            .map(|pattern| {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(context))
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        let icon = if is_prod { "⎈ " } else { "⎈ " };
        let text = format!("{icon}{context}:{namespace}");

        let mut seg = RenderedSegment::new(text).with_cache_key("kubectl_state");
        if is_prod {
            // Signal prod context — consumers can apply red styling via theme color config
            seg.text = format!("[PROD] {}", seg.text);
        }

        Some(seg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ctx_with_kubectl(context: &str, namespace: &str) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            "kubectl_state".to_string(),
            serde_json::json!({ "context": context, "namespace": namespace }),
        );
        RenderContext {
            cwd: "/tmp".to_string(),
            shell_context: lynx_core::types::Context::Interactive,
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
        let r = KubectlContextSegment.render(&Default::default(), &empty_ctx());
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_context_is_default() {
        let r = KubectlContextSegment.render(&Default::default(), &ctx_with_kubectl("default", "default"));
        assert!(r.is_none());
    }

    #[test]
    fn shows_context_and_namespace() {
        let r = KubectlContextSegment.render(&Default::default(), &ctx_with_kubectl("staging", "web"));
        assert!(r.is_some());
        let text = r.unwrap().text;
        assert!(text.contains("staging"));
        assert!(text.contains("web"));
    }

    #[test]
    fn prod_pattern_marks_context() {
        let config = SegmentConfig {
            prod_pattern: Some("prod.*".to_string()),
            ..Default::default()
        };
        let r = KubectlContextSegment.render(&config, &ctx_with_kubectl("prod-us-east", "api"));
        assert!(r.is_some());
        assert!(r.unwrap().text.contains("[PROD]"));
    }

    #[test]
    fn non_prod_context_no_marker() {
        let config = SegmentConfig {
            prod_pattern: Some("prod.*".to_string()),
            ..Default::default()
        };
        let r = KubectlContextSegment.render(&config, &ctx_with_kubectl("staging", "api"));
        assert!(r.is_some());
        assert!(!r.unwrap().text.contains("[PROD]"));
    }
}
