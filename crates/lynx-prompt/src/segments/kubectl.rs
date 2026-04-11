use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct KubectlConfig {
    prod_pattern: Option<String>,
}

/// Shows the current kubectl context and namespace.
/// Hidden when kubectl is not installed or no context is active.
/// Turns red when the context name matches prod_pattern (configurable in theme).
pub struct KubectlContextSegment;

impl Segment for KubectlContextSegment {
    fn name(&self) -> &'static str {
        "kubectl_context"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::KUBECTL_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: KubectlConfig = config.clone().try_into().unwrap_or_default();
        let state = ctx.cache.get(crate::cache_keys::KUBECTL_STATE)?;

        let context = state.get("context")?.as_str()?;
        if context.is_empty() || context == "default" {
            return None;
        }

        let namespace = state
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let is_prod = cfg
            .prod_pattern
            .as_deref()
            .map(|pattern| {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(context))
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        let text = format!("⎈ {context}:{namespace}");
        let mut seg = RenderedSegment::new(text).with_cache_key(crate::cache_keys::KUBECTL_STATE);
        if is_prod {
            seg.text = format!("[PROD] {}", seg.text);
        }

        Some(seg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_kubectl(context: &str, namespace: &str) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::KUBECTL_STATE.to_string(),
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
        let r = KubectlContextSegment.render(&empty_config(), &empty_ctx());
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_context_is_default() {
        let r = KubectlContextSegment.render(&empty_config(), &ctx_with_kubectl("default", "default"));
        assert!(r.is_none());
    }

    #[test]
    fn shows_context_and_namespace() {
        let r = KubectlContextSegment.render(&empty_config(), &ctx_with_kubectl("staging", "web"));
        assert!(r.is_some());
        let text = r.unwrap().text;
        assert!(text.contains("staging"));
        assert!(text.contains("web"));
    }

    #[test]
    fn prod_pattern_marks_context() {
        let cfg: toml::Value = toml::from_str(r#"prod_pattern = "prod.*""#).unwrap();
        let r = KubectlContextSegment.render(&cfg, &ctx_with_kubectl("prod-us-east", "api"));
        assert!(r.is_some());
        assert!(r.unwrap().text.contains("[PROD]"));
    }

    #[test]
    fn non_prod_context_no_marker() {
        let cfg: toml::Value = toml::from_str(r#"prod_pattern = "prod.*""#).unwrap();
        let r = KubectlContextSegment.render(&cfg, &ctx_with_kubectl("staging", "api"));
        assert!(r.is_some());
        assert!(!r.unwrap().text.contains("[PROD]"));
    }
}
