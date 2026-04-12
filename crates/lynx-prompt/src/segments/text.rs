use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Renders arbitrary static text. Used for decorative elements like
/// box-drawing characters (╭─, └─), labels, or separator text in prompts.
///
/// Hidden when `content` is empty or not set.
///
/// TOML config:
/// ```toml
/// [segment.text]
/// content = "╭─"
/// color = { fg = "#21c7c7" }
/// ```
pub struct TextSegment;

#[derive(Deserialize, Default)]
struct TextConfig {
    /// The text content to render. Required — segment is hidden if empty.
    content: Option<String>,
}

impl Segment for TextSegment {
    fn name(&self) -> &'static str {
        "text"
    }

    fn render(&self, config: &toml::Value, _ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: TextConfig = config.clone().try_into().unwrap_or_default();
        let content = cfg.content?;
        if content.is_empty() {
            return None;
        }
        Some(RenderedSegment::new(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx() -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[test]
    fn hidden_without_content() {
        let r = TextSegment.render(&empty_config(), &ctx());
        assert!(r.is_none());
    }

    #[test]
    fn hidden_with_empty_content() {
        let cfg: toml::Value = toml::from_str(r#"content = """#).unwrap();
        let r = TextSegment.render(&cfg, &ctx());
        assert!(r.is_none());
    }

    #[test]
    fn renders_content() {
        let cfg: toml::Value = toml::from_str(r#"content = "╭─""#).unwrap();
        let r = TextSegment.render(&cfg, &ctx()).unwrap();
        assert_eq!(r.text, "╭─");
    }

    #[test]
    fn renders_arbitrary_text() {
        let cfg: toml::Value = toml::from_str(r#"content = "Hello, World!""#).unwrap();
        let r = TextSegment.render(&cfg, &ctx()).unwrap();
        assert_eq!(r.text, "Hello, World!");
    }
}
