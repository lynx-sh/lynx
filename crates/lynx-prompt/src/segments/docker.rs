use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the active Docker context. Hidden when context is "default" or unset.
/// Reads from DOCKER_CONTEXT env var — no subprocess.
///
/// TOML config:
/// ```toml
/// [segment.docker]
/// color = { fg = "#0db7ed" }
/// # icon = "🐳"
/// ```
pub struct DockerSegment;

#[derive(Deserialize, Default)]
struct DockerConfig {
    icon: Option<String>,
}

impl Segment for DockerSegment {
    fn name(&self) -> &'static str {
        "docker"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: DockerConfig = config.clone().try_into().unwrap_or_default();

        let context = ctx.env.get("DOCKER_CONTEXT")?;
        if context.is_empty() || context == "default" {
            return None;
        }

        let icon = cfg.icon.unwrap_or_else(|| "\u{f308}".to_string()); // nf-linux-docker
        let text = format!("{icon} {context}");
        Some(RenderedSegment::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_env(pairs: &[(&str, &str)]) -> RenderContext {
        let env = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env,
        }
    }

    #[test]
    fn hidden_when_unset() {
        let r = DockerSegment.render(&empty_config(), &ctx_with_env(&[]));
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_default() {
        let r = DockerSegment.render(
            &empty_config(),
            &ctx_with_env(&[("DOCKER_CONTEXT", "default")]),
        );
        assert!(r.is_none());
    }

    #[test]
    fn shows_non_default_context() {
        let r = DockerSegment
            .render(
                &empty_config(),
                &ctx_with_env(&[("DOCKER_CONTEXT", "remote-prod")]),
            )
            .unwrap();
        assert!(r.text.contains("remote-prod"));
    }
}
