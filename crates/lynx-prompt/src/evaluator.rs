use std::collections::HashMap;

use futures::future::join_all;
use lynx_core::types::Context;
use lynx_theme::schema::{SegmentCondition, Theme};

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Determine whether a segment should be rendered given its raw config and the
/// active shell context. Evaluated before `render` is called.
///
/// Priority order:
/// 1. `show_in` in config — only show in listed contexts (overrides everything)
/// 2. `hide_in` in config — hide in listed contexts
/// 3. `segment.default_hide_in()` — segment's own default exclusions
/// 4. `show_when` condition — must be true to show (evaluated after context gates)
/// 5. `hide_when` condition — hides when true (ignored when show_when is set)
/// 6. No restriction — always show
fn is_visible(config: &toml::Value, seg: &dyn Segment, ctx: &RenderContext) -> bool {
    let ctx_str = match ctx.shell_context {
        Context::Interactive => "interactive",
        Context::Agent => "agent",
        Context::Minimal => "minimal",
    };

    // Context gate: show_in / hide_in / default_hide_in (same priority as before).
    if let Some(show_in) = config.get("show_in").and_then(|v| v.as_array()) {
        if !show_in.iter().any(|s| s.as_str() == Some(ctx_str)) {
            return false;
        }
    } else if let Some(hide_in) = config.get("hide_in").and_then(|v| v.as_array()) {
        if hide_in.iter().any(|s| s.as_str() == Some(ctx_str)) {
            return false;
        }
    } else if seg.default_hide_in().contains(&ctx_str) {
        return false;
    }

    // Condition gate: show_when / hide_when (evaluated after context gate passes).
    let show_when: Option<SegmentCondition> = config
        .get("show_when")
        .and_then(|v| v.clone().try_into().ok());
    if let Some(cond) = &show_when {
        return eval_condition(cond, ctx);
    }

    let hide_when: Option<SegmentCondition> = config
        .get("hide_when")
        .and_then(|v| v.clone().try_into().ok());
    if let Some(cond) = &hide_when {
        return !eval_condition(cond, ctx);
    }

    // Folder filtering: include_folders / exclude_folders (evaluated after conditions).
    if let Some(folders) = config.get("include_folders").and_then(|v| v.as_array()) {
        let matches = folders.iter().any(|f| {
            f.as_str()
                .map(|p| glob_match(p, &ctx.cwd, &ctx.env))
                .unwrap_or(false)
        });
        if !matches {
            return false;
        }
    } else if let Some(folders) = config.get("exclude_folders").and_then(|v| v.as_array()) {
        let excluded = folders.iter().any(|f| {
            f.as_str()
                .map(|p| glob_match(p, &ctx.cwd, &ctx.env))
                .unwrap_or(false)
        });
        if excluded {
            return false;
        }
    }

    true
}

/// Evaluate a `SegmentCondition` against the current `RenderContext`.
/// Pure — no I/O, no shell calls.
fn eval_condition(cond: &SegmentCondition, ctx: &RenderContext) -> bool {
    match cond {
        SegmentCondition::EnvSet { env_set } => ctx
            .env
            .get(env_set.as_str())
            .map(|v| !v.is_empty())
            .unwrap_or(false),
        SegmentCondition::EnvMatches { env_matches } => env_matches.iter().all(|(var, pattern)| {
            ctx.env
                .get(var.as_str())
                .map(|v| glob_match(pattern, v, &ctx.env))
                .unwrap_or(false)
        }),
        SegmentCondition::InGitRepo { in_git_repo } => {
            let has_git = ctx.cache.contains_key(crate::cache_keys::GIT_STATE);
            *in_git_repo == has_git
        }
        SegmentCondition::CwdMatches { cwd_matches } => glob_match(cwd_matches, &ctx.cwd, &ctx.env),
        SegmentCondition::ExitCodeNonzero { exit_code_nonzero } => {
            let is_nonzero = ctx
                .env
                .get(lynx_core::env_vars::LYNX_LAST_EXIT_CODE)
                .map(|v| v != "0" && !v.is_empty())
                .unwrap_or(false);
            *exit_code_nonzero == is_nonzero
        }
        SegmentCondition::CacheIsTrue { cache_is_true } => {
            // Check all cache entries for a boolean field matching the key.
            // Used for conditional colors: cache_is_true = "staged" matches
            // when any cache entry has {"staged": true}.
            ctx.cache.values().any(|v| {
                v.get(cache_is_true.as_str())
                    .and_then(|f| f.as_bool())
                    .unwrap_or(false)
            })
        }
    }
}

/// Match `value` against a glob `pattern` using `*` (match any chars) and `?` (match one char).
/// A leading `~/` in the pattern is expanded using the `HOME` env var.
///
/// Uses a hand-rolled iterative matcher — no regex compilation on every call.
fn glob_match(pattern: &str, value: &str, env: &HashMap<String, String>) -> bool {
    let expanded: String = if let Some(rest) = pattern.strip_prefix("~/") {
        if let Some(home) = env.get("HOME") {
            format!("{home}/{rest}")
        } else {
            pattern.to_string()
        }
    } else {
        pattern.to_string()
    };

    glob_match_str(&expanded, value)
}

/// Iterative glob matcher: `*` matches any sequence of chars, `?` matches exactly one.
fn glob_match_str(pattern: &str, value: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let v: Vec<char> = value.chars().collect();
    let (mut pi, mut vi) = (0usize, 0usize);
    let (mut star_pi, mut star_vi) = (usize::MAX, 0usize);

    while vi < v.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == v[vi]) {
            pi += 1;
            vi += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_pi = pi;
            star_vi = vi;
            pi += 1;
        } else if star_pi != usize::MAX {
            // Backtrack: the star consumes one more character.
            star_vi += 1;
            vi = star_vi;
            pi = star_pi + 1;
        } else {
            return false;
        }
    }

    // Consume any trailing stars.
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }

    pi == p.len()
}

/// Resolve the effective color for a segment, checking `color_when` conditional overrides
/// in order (first match wins) and falling back to the base `color`.
///
/// Returns the merged `SegmentColor` — conditional fields override base fields,
/// unset conditional fields fall through to the base.
pub fn resolve_conditional_color(
    base: &lynx_theme::schema::SegmentColor,
    color_when: &[lynx_theme::schema::ConditionalColor],
    ctx: &RenderContext,
) -> lynx_theme::schema::SegmentColor {
    for cw in color_when {
        if eval_condition(&cw.condition, ctx) {
            // Merge: conditional fields override base, unset fields fall through.
            return lynx_theme::schema::SegmentColor {
                fg: cw.fg.clone().or_else(|| base.fg.clone()),
                bg: cw.bg.clone().or_else(|| base.bg.clone()),
                bold: cw.bold.unwrap_or(base.bold),
            };
        }
    }
    base.clone()
}

/// Run all segments in the given order concurrently and return the non-None results
/// in order.
pub async fn evaluate(
    segments: &[Box<dyn Segment>],
    order: &[String],
    configs: &HashMap<String, toml::Value>,
    ctx: &RenderContext,
) -> Vec<RenderedSegment> {
    let seg_map: HashMap<&str, &dyn Segment> =
        segments.iter().map(|s| (s.name(), s.as_ref())).collect();

    let futures: Vec<
        std::pin::Pin<Box<dyn std::future::Future<Output = Option<RenderedSegment>> + Send>>,
    > = order
        .iter()
        .map(|name| {
            // `custom_*` segments route to the single "custom" Segment impl.
            let seg = seg_map.get(name.as_str()).copied().or_else(|| {
                if name.starts_with("custom_") {
                    seg_map.get("custom").copied()
                } else {
                    None
                }
            });
            let cfg = configs
                .get(name)
                .cloned()
                .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
            let ctx = ctx.clone();
            let name = name.clone();
            Box::pin(async move {
                if let Some(seg) = seg {
                    if !is_visible(&cfg, seg, &ctx) {
                        return None;
                    }
                    seg.render(&cfg, &ctx).map(|mut r| {
                        if r.cache_key.is_none() {
                            r.cache_key = Some(name);
                        }
                        r
                    })
                } else {
                    None
                }
            })
                as std::pin::Pin<
                    Box<dyn std::future::Future<Output = Option<RenderedSegment>> + Send>,
                >
        })
        .collect();

    join_all(futures).await.into_iter().flatten().collect()
}

/// Evaluate all layout orders from a theme.
/// Returns `(left, right, top, continuation)`.
pub async fn evaluate_theme(
    segments: &[Box<dyn Segment>],
    theme: &Theme,
    ctx: &RenderContext,
) -> (
    Vec<RenderedSegment>,
    Vec<RenderedSegment>,
    Vec<RenderedSegment>,
    Vec<RenderedSegment>,
    Vec<RenderedSegment>,
) {
    let left = evaluate(segments, &theme.segments.left.order, &theme.segment, ctx);
    let right = evaluate(segments, &theme.segments.right.order, &theme.segment, ctx);
    let top = evaluate(segments, &theme.segments.top.order, &theme.segment, ctx);
    let top_right = evaluate(
        segments,
        &theme.segments.top_right.order,
        &theme.segment,
        ctx,
    );
    let continuation = evaluate(
        segments,
        &theme.segments.continuation.order,
        &theme.segment,
        ctx,
    );
    let (left, right, top, top_right, continuation) =
        tokio::join!(left, right, top, top_right, continuation);
    (left, right, top, top_right, continuation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_core::types::Context;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use crate::segment::{RenderContext, RenderedSegment, Segment};

    /// A segment that sleeps for a fixed duration then returns a value.
    struct SlowSegment {
        name: &'static str,
        delay_ms: u64,
    }

    impl Segment for SlowSegment {
        fn name(&self) -> &'static str {
            self.name
        }

        fn render(&self, _config: &toml::Value, _ctx: &RenderContext) -> Option<RenderedSegment> {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
            Some(RenderedSegment::new(self.name))
        }
    }

    fn ctx() -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context: Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn segments_evaluated_concurrently() {
        // Two segments each sleeping 100ms. If sequential, total ≥ 200ms.
        // If concurrent, total ≈ 100ms (the slowest).
        let segments: Vec<Box<dyn Segment>> = vec![
            Box::new(SlowSegment {
                name: "a",
                delay_ms: 100,
            }),
            Box::new(SlowSegment {
                name: "b",
                delay_ms: 100,
            }),
        ];
        let order = vec!["a".to_string(), "b".to_string()];
        let start = Instant::now();
        let results = evaluate(&segments, &order, &HashMap::new(), &ctx()).await;
        let elapsed = start.elapsed();

        assert_eq!(results.len(), 2);
        // With real parallelism this would be ~100ms; allow generous 350ms for CI.
        assert!(
            elapsed < Duration::from_millis(350),
            "segments should run concurrently, got {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn none_segments_are_filtered() {
        // Verify filter works for unknown segment name.
        let segments: Vec<Box<dyn Segment>> = vec![];
        let order = vec!["nonexistent".to_string()];
        let results = evaluate(&segments, &order, &HashMap::new(), &ctx()).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn order_preserved() {
        let segments: Vec<Box<dyn Segment>> = vec![
            Box::new(SlowSegment {
                name: "slow",
                delay_ms: 80,
            }),
            Box::new(SlowSegment {
                name: "fast",
                delay_ms: 10,
            }),
        ];
        let order = vec!["slow".to_string(), "fast".to_string()];
        let results = evaluate(&segments, &order, &HashMap::new(), &ctx()).await;
        assert_eq!(results[0].text, "slow");
        assert_eq!(results[1].text, "fast");
    }

    #[tokio::test]
    async fn custom_prefix_routes_to_custom_segment() {
        use crate::segments::CustomSegment;

        let segments: Vec<Box<dyn Segment>> = vec![Box::new(CustomSegment)];
        let order = vec!["custom_greeting".to_string()];

        let mut cfg_map = HashMap::new();
        let mut seg_cfg = toml::map::Map::new();
        seg_cfg.insert(
            "template".to_string(),
            toml::Value::String("hello $cwd".to_string()),
        );
        cfg_map.insert("custom_greeting".to_string(), toml::Value::Table(seg_cfg));

        let mut ctx = ctx();
        ctx.cwd = "/home/test".to_string();

        let results = evaluate(&segments, &order, &cfg_map, &ctx).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "hello /home/test");
    }

    // ── show_when / hide_when condition tests ────────────────────────────────

    struct AlwaysSegment;
    impl Segment for AlwaysSegment {
        fn name(&self) -> &'static str {
            "always"
        }
        fn render(&self, _: &toml::Value, _: &RenderContext) -> Option<RenderedSegment> {
            Some(RenderedSegment::new("ok"))
        }
    }

    fn cfg_from_toml(s: &str) -> toml::Value {
        toml::from_str::<toml::Value>(&format!("[seg]\n{s}")).unwrap()["seg"].clone()
    }

    #[test]
    fn show_when_env_set_present() {
        let mut ctx = ctx();
        ctx.env.insert("SSH_CONNECTION".into(), "user@host".into());
        let cfg = cfg_from_toml(r#"show_when = { env_set = "SSH_CONNECTION" }"#);
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_env_set_absent() {
        let ctx = ctx(); // SSH_CONNECTION not in env
        let cfg = cfg_from_toml(r#"show_when = { env_set = "SSH_CONNECTION" }"#);
        assert!(!is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_env_set_empty_string_counts_as_absent() {
        let mut ctx = ctx();
        ctx.env.insert("SSH_CONNECTION".into(), "".into());
        let cfg = cfg_from_toml(r#"show_when = { env_set = "SSH_CONNECTION" }"#);
        assert!(!is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_env_matches_glob() {
        let mut ctx = ctx();
        ctx.env
            .insert("VIRTUAL_ENV".into(), "/home/user/.venv/myproject".into());
        let cfg = cfg_from_toml(r#"show_when = { env_matches = { VIRTUAL_ENV = "*myproject*" } }"#);
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_env_matches_glob_no_match() {
        let mut ctx = ctx();
        ctx.env
            .insert("VIRTUAL_ENV".into(), "/home/user/.venv/other".into());
        let cfg = cfg_from_toml(r#"show_when = { env_matches = { VIRTUAL_ENV = "*myproject*" } }"#);
        assert!(!is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_in_git_repo_true_with_git_cache() {
        let mut ctx = ctx();
        ctx.cache.insert(
            crate::cache_keys::GIT_STATE.into(),
            serde_json::json!({"branch": "main"}),
        );
        let cfg = cfg_from_toml(r#"show_when = { in_git_repo = true }"#);
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_in_git_repo_true_without_git_cache() {
        let ctx = ctx(); // no GIT_STATE in cache
        let cfg = cfg_from_toml(r#"show_when = { in_git_repo = true }"#);
        assert!(!is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_in_git_repo_false_outside_repo() {
        let ctx = ctx(); // no GIT_STATE = not in a git repo
        let cfg = cfg_from_toml(r#"show_when = { in_git_repo = false }"#);
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_cwd_matches_glob() {
        let mut ctx = ctx();
        ctx.cwd = "/home/user/work/project".into();
        let cfg = cfg_from_toml(r#"show_when = { cwd_matches = "/home/user/work/**" }"#);
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_cwd_matches_tilde_expansion() {
        let mut ctx = ctx();
        ctx.cwd = "/home/user/work/project".into();
        ctx.env.insert("HOME".into(), "/home/user".into());
        let cfg = cfg_from_toml(r#"show_when = { cwd_matches = "~/work/**" }"#);
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_cwd_matches_no_match() {
        let mut ctx = ctx();
        ctx.cwd = "/tmp/other".into();
        ctx.env.insert("HOME".into(), "/home/user".into());
        let cfg = cfg_from_toml(r#"show_when = { cwd_matches = "~/work/**" }"#);
        assert!(!is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_exit_code_nonzero_true() {
        let mut ctx = ctx();
        ctx.env
            .insert(lynx_core::env_vars::LYNX_LAST_EXIT_CODE.into(), "1".into());
        let cfg = cfg_from_toml(r#"show_when = { exit_code_nonzero = true }"#);
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_exit_code_nonzero_false_on_zero() {
        let mut ctx = ctx();
        ctx.env
            .insert(lynx_core::env_vars::LYNX_LAST_EXIT_CODE.into(), "0".into());
        let cfg = cfg_from_toml(r#"show_when = { exit_code_nonzero = true }"#);
        assert!(!is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn hide_when_env_set_hides_segment() {
        let mut ctx = ctx();
        ctx.env.insert("CI".into(), "true".into());
        let cfg = cfg_from_toml(r#"hide_when = { env_set = "CI" }"#);
        assert!(!is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn hide_when_env_set_shows_when_absent() {
        let ctx = ctx(); // CI not set
        let cfg = cfg_from_toml(r#"hide_when = { env_set = "CI" }"#);
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }

    #[test]
    fn show_when_takes_priority_over_hide_when() {
        // If both show_when and hide_when are set, show_when wins.
        let mut ctx = ctx();
        ctx.env.insert("SSH_CONNECTION".into(), "x".into());
        ctx.env.insert("CI".into(), "true".into());
        // show_when passes (SSH_CONNECTION set), hide_when would also trigger (CI set)
        // but show_when takes priority.
        let cfg = cfg_from_toml(
            r#"show_when = { env_set = "SSH_CONNECTION" }
hide_when = { env_set = "CI" }"#,
        );
        assert!(is_visible(&cfg, &AlwaysSegment, &ctx));
    }
}
