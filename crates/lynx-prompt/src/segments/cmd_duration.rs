use lynx_theme::schema::SegmentConfig;

use crate::segment::{RenderContext, RenderedSegment, Segment};

pub struct CmdDurationSegment;

impl Segment for CmdDurationSegment {
    fn name(&self) -> &'static str {
        "cmd_duration"
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        let ms = ctx.last_cmd_ms?;
        let threshold = config.min_ms.unwrap_or(500);
        if ms < threshold {
            return None;
        }
        let display = format_duration(ms);
        Some(RenderedSegment::new(display))
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
    use std::collections::HashMap;

    fn ctx(ms: Option<u64>) -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: ms,
            cache: HashMap::new(),
        }
    }

    #[test]
    fn hides_under_threshold() {
        let cfg = SegmentConfig {
            min_ms: Some(500),
            ..Default::default()
        };
        assert!(CmdDurationSegment.render(&cfg, &ctx(Some(200))).is_none());
    }

    #[test]
    fn shows_when_over_threshold() {
        let cfg = SegmentConfig {
            min_ms: Some(500),
            ..Default::default()
        };
        assert!(CmdDurationSegment.render(&cfg, &ctx(Some(1500))).is_some());
    }

    #[test]
    fn hides_when_no_duration() {
        assert!(CmdDurationSegment
            .render(&Default::default(), &ctx(None))
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
