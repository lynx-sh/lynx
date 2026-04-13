use serde::Deserialize;

use crate::segment::{apply_format, RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct GitAheadBehindConfig {
    /// Symbol for ahead count. Default: "↑".
    ahead_symbol: Option<String>,
    /// Symbol for behind count. Default: "↓".
    behind_symbol: Option<String>,
    /// Format template. Available vars: `$ahead`, `$behind`.
    /// Each expands to `<symbol><count>` when non-zero, or empty string.
    /// Default: `"$ahead $behind"` (space-joined, trimmed).
    format: Option<String>,
}

pub struct GitAheadBehindSegment;

use super::git_common::git_state_obj;

impl Segment for GitAheadBehindSegment {
    fn name(&self) -> &'static str {
        "git_ahead_behind"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GIT_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GitAheadBehindConfig = config.clone().try_into().unwrap_or_default();
        let obj = git_state_obj(ctx)?;

        let ahead = obj.get("ahead").and_then(|v| v.as_u64()).unwrap_or(0);
        let behind = obj.get("behind").and_then(|v| v.as_u64()).unwrap_or(0);

        if ahead == 0 && behind == 0 {
            return None;
        }

        let ahead_sym = cfg.ahead_symbol.unwrap_or_else(|| "↑".to_string());
        let behind_sym = cfg.behind_symbol.unwrap_or_else(|| "↓".to_string());

        let ahead_str = if ahead > 0 { format!("{ahead_sym}{ahead}") } else { String::new() };
        let behind_str = if behind > 0 { format!("{behind_sym}{behind}") } else { String::new() };

        let text = match cfg.format.as_deref() {
            Some(tmpl) => {
                let s = apply_format(tmpl, &[("ahead", &ahead_str), ("behind", &behind_str)]);
                // Trim leading/trailing whitespace that results from empty vars.
                s.trim().to_string()
            }
            None => {
                let mut parts: Vec<&str> = Vec::new();
                if !ahead_str.is_empty() { parts.push(&ahead_str); }
                if !behind_str.is_empty() { parts.push(&behind_str); }
                parts.join(" ")
            }
        };

        if text.is_empty() {
            return None;
        }

        Some(RenderedSegment::new(text).with_cache_key("git_ahead_behind"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_counts(ahead: u64, behind: u64) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::GIT_STATE.into(),
            serde_json::json!({ "branch": "main", "ahead": ahead, "behind": behind }),
        );
        RenderContext {
            cwd: "/repo".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
            env: HashMap::new(),
        }
    }

    #[test]
    fn hidden_when_both_zero() {
        let ctx = ctx_with_counts(0, 0);
        let r = GitAheadBehindSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_ahead_only() {
        let ctx = ctx_with_counts(2, 0);
        let r = GitAheadBehindSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "↑2");
    }

    #[test]
    fn shows_behind_only() {
        let ctx = ctx_with_counts(0, 3);
        let r = GitAheadBehindSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "↓3");
    }

    #[test]
    fn shows_both() {
        let ctx = ctx_with_counts(1, 2);
        let r = GitAheadBehindSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "↑1 ↓2");
    }

    #[test]
    fn custom_symbols() {
        let cfg: toml::Value = toml::from_str(r#"ahead_symbol = "⇡"
behind_symbol = "⇣""#).unwrap();
        let ctx = ctx_with_counts(3, 1);
        let r = GitAheadBehindSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "⇡3 ⇣1");
    }
}
