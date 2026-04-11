use chrono::Local;
use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct TimeConfig {
    /// Clock format: `"12h"` or `"24h"`. Default: `"24h"`.
    format: Option<String>,
}

pub struct TimeSegment;

impl Segment for TimeSegment {
    fn name(&self) -> &'static str {
        "time"
    }

    fn render(&self, config: &toml::Value, _ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: TimeConfig = config.clone().try_into().unwrap_or_default();
        let fmt = match cfg.format.as_deref() {
            Some("12h") => "%I:%M %p",
            _ => "%H:%M",
        };
        let text = Local::now().format(fmt).to_string();
        Some(RenderedSegment::new(text))
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
    fn renders_24h_format_by_default() {
        let r = TimeSegment.render(&empty_config(), &ctx()).unwrap();
        // HH:MM — two digits, colon, two digits
        assert!(
            r.text.len() == 5 && r.text.chars().nth(2) == Some(':'),
            "unexpected 24h format: {}",
            r.text
        );
    }

    #[test]
    fn renders_12h_format() {
        let cfg: toml::Value = toml::from_str(r#"format = "12h""#).unwrap();
        let r = TimeSegment.render(&cfg, &ctx()).unwrap();
        // Should contain AM or PM
        assert!(
            r.text.contains("AM") || r.text.contains("PM"),
            "expected AM/PM in 12h output: {}",
            r.text
        );
    }
}
