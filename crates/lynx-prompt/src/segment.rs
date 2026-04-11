use std::collections::HashMap;

use lynx_core::types::Context;
use lynx_theme::schema::SegmentConfig;

/// Data passed to every segment at render time.
#[derive(Debug, Clone)]
pub struct RenderContext {
    /// Current working directory (absolute path).
    pub cwd: String,
    /// Shell context (interactive / agent / minimal).
    pub shell_context: Context,
    /// Duration of the last command in milliseconds.
    pub last_cmd_ms: Option<u64>,
    /// Shared segment cache (keyed by cache key).
    pub cache: HashMap<String, serde_json::Value>,
}

/// A rendered segment ready for display.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedSegment {
    /// The display text (may contain ANSI codes).
    pub text: String,
    /// Cache key this segment reads from (if any). Declared for cache wiring.
    pub cache_key: Option<String>,
}

impl RenderedSegment {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cache_key: None,
        }
    }

    pub fn with_cache_key(mut self, key: impl Into<String>) -> Self {
        self.cache_key = Some(key.into());
        self
    }
}

/// All segment implementations must implement this trait.
///
/// `render` returns `None` when the segment should be hidden entirely.
/// Segments MUST NOT perform blocking I/O — slow data must come from the cache.
pub trait Segment: Send + Sync {
    fn name(&self) -> &'static str;

    /// Cache key this segment reads, if any. Returned as metadata for wiring.
    fn cache_key(&self) -> Option<&'static str> {
        None
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment>;
}
