use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the active Google Cloud project. Hidden when unset.
/// Reads CLOUDSDK_CORE_PROJECT or GCLOUD_PROJECT env vars — no subprocess.
///
/// TOML config:
/// ```toml
/// [segment.gcp]
/// color = { fg = "#4285f4" }
/// # icon = "☁"
/// ```
pub struct GcpSegment;

#[derive(Deserialize, Default)]
struct GcpConfig {
    icon: Option<String>,
}

impl Segment for GcpSegment {
    fn name(&self) -> &'static str {
        "gcp"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GcpConfig = config.clone().try_into().unwrap_or_default();

        let project = ctx
            .env
            .get("CLOUDSDK_CORE_PROJECT")
            .or_else(|| ctx.env.get("GCLOUD_PROJECT"))
            .filter(|v| !v.is_empty())?;

        let icon = cfg.icon.unwrap_or_else(|| "\u{f7b7}".to_string()); // nf-md-google_cloud
        let text = format!("{icon} {project}");
        Some(RenderedSegment::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_env(pairs: &[(&str, &str)]) -> RenderContext {
        let env = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
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
        let r = GcpSegment.render(&empty_config(), &ctx_with_env(&[]));
        assert!(r.is_none());
    }

    #[test]
    fn shows_cloudsdk_project() {
        let r = GcpSegment.render(&empty_config(), &ctx_with_env(&[("CLOUDSDK_CORE_PROJECT", "my-project")])).unwrap();
        assert!(r.text.contains("my-project"));
    }

    #[test]
    fn shows_gcloud_project_fallback() {
        let r = GcpSegment.render(&empty_config(), &ctx_with_env(&[("GCLOUD_PROJECT", "fallback-proj")])).unwrap();
        assert!(r.text.contains("fallback-proj"));
    }

    #[test]
    fn hidden_when_empty() {
        let r = GcpSegment.render(&empty_config(), &ctx_with_env(&[("CLOUDSDK_CORE_PROJECT", "")]));
        assert!(r.is_none());
    }
}
