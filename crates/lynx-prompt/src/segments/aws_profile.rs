use serde::Deserialize;

use crate::color_apply::colorize;
use crate::segment::{RenderContext, RenderedSegment, Segment};
use lynx_theme::schema::SegmentColor;

#[derive(Deserialize, Default)]
struct AwsProfileConfig {
    /// Glob-like patterns that identify production profiles. Default: ["*prod*", "*production*"].
    prod_patterns: Option<Vec<String>>,
    /// Color for non-production profiles. Default: "green".
    color: Option<String>,
    /// Color for production profiles. Default: "red".
    prod_color: Option<String>,
    /// Icon prepended to the profile name. Default: empty.
    icon: Option<String>,
}

pub struct AwsProfileSegment;

impl Segment for AwsProfileSegment {
    fn name(&self) -> &'static str {
        "aws_profile"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: AwsProfileConfig = config.clone().try_into().unwrap_or_default();

        let profile = ctx.env.get("AWS_PROFILE")?;
        if profile.is_empty() {
            return None;
        }

        let patterns = cfg
            .prod_patterns
            .unwrap_or_else(|| vec!["*prod*".to_string(), "*production*".to_string()]);

        let is_prod = patterns.iter().any(|p| {
            let needle = p.trim_matches('*');
            if needle.is_empty() {
                false
            } else {
                profile.to_lowercase().contains(&needle.to_lowercase())
            }
        });

        let color_name = if is_prod {
            cfg.prod_color.unwrap_or_else(|| "red".to_string())
        } else {
            cfg.color.unwrap_or_else(|| "green".to_string())
        };

        let icon = cfg.icon.unwrap_or_default();
        let text = if icon.is_empty() {
            profile.to_string()
        } else {
            format!("{icon} {profile}")
        };

        let colored = colorize(
            &text,
            &SegmentColor {
                fg: Some(color_name),
                bg: None,
                bold: is_prod,
            },
        );

        Some(RenderedSegment::new(&colored).with_cache_key("aws_profile"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use lynx_theme::terminal::{override_capability, TermCapability};
    use std::collections::HashMap;

    fn ctx_with_profile(profile: &str) -> RenderContext {
        let mut env = HashMap::new();
        env.insert("AWS_PROFILE".to_string(), profile.to_string());
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env,
        }
    }

    #[test]
    fn hidden_when_absent() {
        let ctx = RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        };
        assert!(AwsProfileSegment.render(&empty_config(), &ctx).is_none());
    }

    #[test]
    fn hidden_when_empty() {
        let ctx = ctx_with_profile("");
        assert!(AwsProfileSegment.render(&empty_config(), &ctx).is_none());
    }

    #[test]
    fn shows_staging_profile() {
        override_capability(TermCapability::TrueColor);
        let ctx = ctx_with_profile("staging");
        let seg = AwsProfileSegment.render(&empty_config(), &ctx).unwrap();
        assert!(
            seg.text.contains("staging"),
            "expected profile name: {:?}",
            seg.text
        );
        // green = (158,206,106) → 38;2;158;206;106
        assert!(
            seg.text.contains("38;2;158;206;106"),
            "expected green color: {:?}",
            seg.text
        );
    }

    #[test]
    fn prod_profile_detected() {
        override_capability(TermCapability::TrueColor);
        let ctx = ctx_with_profile("my-prod-account");
        let seg = AwsProfileSegment.render(&empty_config(), &ctx).unwrap();
        assert!(
            seg.text.contains("my-prod-account"),
            "expected profile: {:?}",
            seg.text
        );
        // red = (247,118,142) → 38;2;247;118;142
        assert!(
            seg.text.contains("38;2;247;118;142"),
            "expected red color: {:?}",
            seg.text
        );
        // bold
        assert!(
            seg.text.contains("\x1b[1m"),
            "expected bold for prod: {:?}",
            seg.text
        );
    }

    #[test]
    fn production_pattern_matches() {
        override_capability(TermCapability::TrueColor);
        let ctx = ctx_with_profile("us-east-production");
        let seg = AwsProfileSegment.render(&empty_config(), &ctx).unwrap();
        assert!(
            seg.text.contains("38;2;247;118;142"),
            "expected red for production: {:?}",
            seg.text
        );
    }

    #[test]
    fn custom_icon() {
        override_capability(TermCapability::TrueColor);
        let cfg: toml::Value = toml::from_str(r#"icon = "☁""#).unwrap();
        let ctx = ctx_with_profile("dev");
        let seg = AwsProfileSegment.render(&cfg, &ctx).unwrap();
        assert!(seg.text.contains("☁"), "expected icon: {:?}", seg.text);
        assert!(seg.text.contains("dev"), "expected profile: {:?}", seg.text);
    }

    #[test]
    fn cache_key_is_aws_profile() {
        override_capability(TermCapability::TrueColor);
        let ctx = ctx_with_profile("dev");
        let seg = AwsProfileSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(seg.cache_key.as_deref(), Some("aws_profile"));
    }
}
