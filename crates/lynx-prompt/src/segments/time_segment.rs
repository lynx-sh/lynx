use chrono::Local;
use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct TimeConfig {
    /// Clock format: `"12h"`, `"24h"`, or a custom strftime pattern (e.g. `"%Y-%m-%d %H:%M:%S"`).
    /// Default: `"24h"` → `%H:%M`.
    format: Option<String>,
    /// Optional prefix icon (e.g. `"⏱ "`).
    icon: Option<String>,
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
            Some("24h") | None => "%H:%M",
            Some(custom) => custom,
        };
        let time_str = Local::now().format(fmt).to_string();
        let text = match cfg.icon.as_deref() {
            Some(icon) => format!("{icon}{time_str}"),
            None => time_str,
        };
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

    #[test]
    fn renders_custom_strftime_format() {
        let cfg: toml::Value = toml::from_str(r#"format = "%Y-%m-%d %H:%M:%S""#).unwrap();
        let r = TimeSegment.render(&cfg, &ctx()).unwrap();
        // YYYY-MM-DD HH:MM:SS — 19 chars, dash at pos 4 and 7
        assert_eq!(r.text.len(), 19, "unexpected length: {}", r.text);
        assert_eq!(
            r.text.chars().nth(4),
            Some('-'),
            "expected dash at pos 4: {}",
            r.text
        );
        assert_eq!(
            r.text.chars().nth(10),
            Some(' '),
            "expected space at pos 10: {}",
            r.text
        );
    }
}
