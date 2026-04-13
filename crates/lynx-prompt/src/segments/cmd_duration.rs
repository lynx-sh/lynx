use serde::Deserialize;

use crate::segment::{apply_format, RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct CmdDurationConfig {
    min_ms: Option<u64>,
    /// Format template. Available vars: `$duration`.
    /// Default: `"$duration"`.
    format: Option<String>,
}

pub struct CmdDurationSegment;

impl Segment for CmdDurationSegment {
    fn name(&self) -> &'static str {
        "cmd_duration"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: CmdDurationConfig = config.clone().try_into().unwrap_or_default();
        let ms = ctx.last_cmd_ms?;
        let threshold = cfg.min_ms.unwrap_or(500);
        if ms < threshold {
            return None;
        }
        let dur = format_duration(ms);
        let text = match cfg.format.as_deref() {
            Some(tmpl) => apply_format(tmpl, &[("duration", &dur)]),
            None => dur,
        };
        Some(RenderedSegment::new(text))
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        let secs = ms / 1_000;
        format!("{}m{}s", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx(ms: Option<u64>) -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: ms,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    fn cfg(s: &str) -> toml::Value {
        toml::from_str(s).unwrap()
    }

    #[test]
    fn hides_under_threshold() {
        assert!(CmdDurationSegment
            .render(&cfg("min_ms = 500"), &ctx(Some(200)))
            .is_none());
    }

    #[test]
    fn shows_when_over_threshold() {
        assert!(CmdDurationSegment
            .render(&cfg("min_ms = 500"), &ctx(Some(1500)))
            .is_some());
    }

    #[test]
    fn hides_when_no_duration() {
        assert!(CmdDurationSegment
            .render(&empty_config(), &ctx(None))
            .is_none());
    }

    #[test]
    fn formats_milliseconds() {
        assert_eq!(format_duration(300), "300ms");
    }

    #[test]
    fn formats_seconds() {
        assert_eq!(format_duration(2500), "2.5s");
    }

    #[test]
    fn formats_minutes() {
        assert_eq!(format_duration(90_000), "1m30s");
    }
}
