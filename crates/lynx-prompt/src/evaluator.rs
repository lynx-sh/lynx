use std::collections::HashMap;

use futures::future::join_all;
use lynx_core::types::Context;
use lynx_theme::schema::Theme;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Determine whether a segment should be rendered given its raw config and the
/// active shell context. Evaluated before `render` is called.
///
/// Priority order:
/// 1. `show_in` in config — only show in listed contexts (overrides everything)
/// 2. `hide_in` in config — hide in listed contexts
/// 3. `segment.default_hide_in()` — segment's own default exclusions
/// 4. No restriction — always show
fn is_visible(config: &toml::Value, seg: &dyn Segment, ctx: &RenderContext) -> bool {
    let ctx_str = match ctx.shell_context {
        Context::Interactive => "interactive",
        Context::Agent => "agent",
        Context::Minimal => "minimal",
    };

    if let Some(show_in) = config.get("show_in").and_then(|v| v.as_array()) {
        return show_in.iter().any(|s| s.as_str() == Some(ctx_str));
    }
    if let Some(hide_in) = config.get("hide_in").and_then(|v| v.as_array()) {
        return !hide_in.iter().any(|s| s.as_str() == Some(ctx_str));
    }
    !seg.default_hide_in().contains(&ctx_str)
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
) {
    let left = evaluate(segments, &theme.segments.left.order, &theme.segment, ctx);
    let right = evaluate(segments, &theme.segments.right.order, &theme.segment, ctx);
    let top = evaluate(segments, &theme.segments.top.order, &theme.segment, ctx);
    let continuation = evaluate(
        segments,
        &theme.segments.continuation.order,
        &theme.segment,
        ctx,
    );
    tokio::join!(left, right, top, continuation)
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
}
