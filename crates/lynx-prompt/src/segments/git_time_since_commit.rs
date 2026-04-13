use serde::Deserialize;

use crate::color_apply::colorize;
use crate::segment::{RenderContext, RenderedSegment, Segment};
use lynx_theme::schema::SegmentColor;

#[derive(Deserialize, Default)]
struct GitTimeSinceCommitConfig {
    /// Seconds before the time is considered "fresh" (green). Default: 600 (10 min).
    fresh_secs: Option<u64>,
    /// Seconds before the time is considered "warn" (yellow). Default: 1800 (30 min).
    warn_secs: Option<u64>,
    /// Color name/hex for fresh commits. Default: "green".
    fresh_color: Option<String>,
    /// Color name/hex for warning-age commits. Default: "yellow".
    warn_color: Option<String>,
    /// Color name/hex for old commits. Default: "red".
    old_color: Option<String>,
}

pub struct GitTimeSinceCommitSegment;

impl Segment for GitTimeSinceCommitSegment {
    fn name(&self) -> &'static str {
        "git_time_since_commit"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GIT_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GitTimeSinceCommitConfig = config.clone().try_into().unwrap_or_default();

        let commit_ts = git_state_u64(ctx, "commit_ts")?;
        let now_secs = ctx
            .env
            .get("LYNX_NOW_SECS")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        if now_secs == 0 || now_secs < commit_ts {
            return None;
        }

        let elapsed = now_secs - commit_ts;
        let text = format_elapsed(elapsed);

        let fresh_secs = cfg.fresh_secs.unwrap_or(600);
        let warn_secs = cfg.warn_secs.unwrap_or(1800);

        let color = if elapsed < fresh_secs {
            cfg.fresh_color.unwrap_or_else(|| "green".to_string())
        } else if elapsed < warn_secs {
            cfg.warn_color.unwrap_or_else(|| "yellow".to_string())
        } else {
            cfg.old_color.unwrap_or_else(|| "red".to_string())
        };

        let colored_text = colorize(
            &text,
            &SegmentColor {
                fg: Some(color),
                bg: None,
                bold: false,
            },
        );

        Some(
            RenderedSegment::new(&colored_text)
                .with_cache_key("git_time_since_commit"),
        )
    }
}

fn git_state_u64(ctx: &RenderContext, key: &str) -> Option<u64> {
    match ctx.cache.get(crate::cache_keys::GIT_STATE)? {
        serde_json::Value::Object(m) => m.get(key)?.as_u64(),
        _ => None,
    }
}

fn format_elapsed(secs: u64) -> String {
    let minutes = secs / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{days}d")
    } else if hours > 0 {
        format!("{}h{}m", hours, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use lynx_theme::terminal::override_capability;
    use lynx_theme::terminal::TermCapability;
    use std::collections::HashMap;

    fn ctx_with_ts(commit_ts: u64, now_secs: u64) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::GIT_STATE.into(),
            serde_json::json!({"branch": "main", "commit_ts": commit_ts}),
        );
        let mut env = HashMap::new();
        env.insert("LYNX_NOW_SECS".to_string(), now_secs.to_string());
        RenderContext {
            cwd: "/repo".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
            env,
        }
    }

    #[test]
    fn hidden_when_no_git_state() {
        let ctx = RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        };
        assert!(GitTimeSinceCommitSegment.render(&empty_config(), &ctx).is_none());
    }

    #[test]
    fn hidden_when_no_now_secs() {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::GIT_STATE.into(),
            serde_json::json!({"branch": "main", "commit_ts": 1000}),
        );
        let ctx = RenderContext {
            cwd: "/repo".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
            env: HashMap::new(),
        };
        assert!(GitTimeSinceCommitSegment.render(&empty_config(), &ctx).is_none());
    }

    #[test]
    fn fresh_5_minutes() {
        override_capability(TermCapability::TrueColor);
        let ctx = ctx_with_ts(1000, 1300); // 300s = 5m
        let seg = GitTimeSinceCommitSegment.render(&empty_config(), &ctx).unwrap();
        assert!(seg.text.contains("5m"), "expected '5m' in: {:?}", seg.text);
        // fresh = green, named_to_rgb("green") = (158,206,106) → 38;2;158;206;106
        assert!(seg.text.contains("38;2;158;206;106"), "expected green color in: {:?}", seg.text);
    }

    #[test]
    fn warn_20_minutes() {
        override_capability(TermCapability::TrueColor);
        let ctx = ctx_with_ts(1000, 2200); // 1200s = 20m
        let seg = GitTimeSinceCommitSegment.render(&empty_config(), &ctx).unwrap();
        assert!(seg.text.contains("20m"), "expected '20m' in: {:?}", seg.text);
        // warn = yellow, named_to_rgb("yellow") = (224,175,104)
        assert!(seg.text.contains("38;2;224;175;104"), "expected yellow color in: {:?}", seg.text);
    }

    #[test]
    fn old_2_hours() {
        override_capability(TermCapability::TrueColor);
        let ctx = ctx_with_ts(1000, 8200); // 7200s = 2h
        let seg = GitTimeSinceCommitSegment.render(&empty_config(), &ctx).unwrap();
        assert!(seg.text.contains("2h0m"), "expected '2h0m' in: {:?}", seg.text);
        // old = red, named_to_rgb("red") = (247,118,142)
        assert!(seg.text.contains("38;2;247;118;142"), "expected red color in: {:?}", seg.text);
    }

    #[test]
    fn days_format() {
        let ctx = ctx_with_ts(1000, 1000 + 86400 * 3 + 3600); // 3d+1h
        let seg = GitTimeSinceCommitSegment.render(&empty_config(), &ctx).unwrap();
        assert!(seg.text.contains("3d"), "expected '3d' in: {:?}", seg.text);
    }

    #[test]
    fn cache_key_is_correct() {
        let ctx = ctx_with_ts(1000, 1300);
        let seg = GitTimeSinceCommitSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(seg.cache_key.as_deref(), Some("git_time_since_commit"));
    }

    #[test]
    fn format_elapsed_unit() {
        assert_eq!(format_elapsed(0), "0m");
        assert_eq!(format_elapsed(59), "0m");
        assert_eq!(format_elapsed(60), "1m");
        assert_eq!(format_elapsed(3600), "1h0m");
        assert_eq!(format_elapsed(3660), "1h1m");
        assert_eq!(format_elapsed(86400), "1d");
        assert_eq!(format_elapsed(90000), "1d");
    }
}
