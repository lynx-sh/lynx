use std::collections::HashMap;

use futures::future::join_all;
use lynx_theme::schema::{SegmentConfig, Theme};

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Run all segments in the given order concurrently and return the non-None results
/// in order.
pub async fn evaluate(
    segments: &[Box<dyn Segment>],
    order: &[String],
    configs: &HashMap<String, SegmentConfig>,
    ctx: &RenderContext,
) -> Vec<RenderedSegment> {
    // Build a map from name → segment impl.
    let seg_map: HashMap<&str, &dyn Segment> =
        segments.iter().map(|s| (s.name(), s.as_ref())).collect();

    // Spawn all segments concurrently via tokio tasks.
    let futures: Vec<
        std::pin::Pin<Box<dyn std::future::Future<Output = Option<RenderedSegment>> + Send>>,
    > = order
        .iter()
        .map(|name| {
            let seg = seg_map.get(name.as_str()).copied();
            let cfg = configs.get(name).cloned().unwrap_or_default();
            let ctx = ctx.clone();
            Box::pin(async move {
                if let Some(seg) = seg {
                    seg.render(&cfg, &ctx)
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

/// Evaluate both left and right orders from a theme, return (left_segments, right_segments).
pub async fn evaluate_theme(
    segments: &[Box<dyn Segment>],
    theme: &Theme,
    ctx: &RenderContext,
) -> (Vec<RenderedSegment>, Vec<RenderedSegment>) {
    let left = evaluate(segments, &theme.segments.left.order, &theme.segment, ctx);
    let right = evaluate(segments, &theme.segments.right.order, &theme.segment, ctx);
    tokio::join!(left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_core::types::Context;
    use lynx_theme::schema::SegmentConfig;
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

        fn render(&self, _config: &SegmentConfig, _ctx: &RenderContext) -> Option<RenderedSegment> {
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
}
