use std::collections::HashMap;

use lynx_core::types::Context;

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
    /// Snapshot of relevant environment variables captured before render.
    /// Segments must read env via this field — never call std::env::var() directly.
    pub env: HashMap<String, String>,
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

/// Return an empty segment config (no fields set). Convenience for tests.
pub fn empty_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

/// All segment implementations must implement this trait.
///
/// `render` receives the raw TOML table for this segment. Each segment
/// deserializes its own typed config from it. Universal fields (`show_in`,
/// `hide_in`) are handled by the evaluator before render is called.
///
/// `render` returns `None` when the segment should be hidden entirely.
/// Segments MUST NOT perform blocking I/O — slow data must come from the cache.
pub trait Segment: Send + Sync {
    fn name(&self) -> &'static str;

    /// Cache key this segment reads, if any. Returned as metadata for wiring.
    fn cache_key(&self) -> Option<&'static str> {
        None
    }

    /// Contexts this segment hides in by default when no `hide_in` or `show_in`
    /// is set in config. The evaluator checks this before calling render.
    fn default_hide_in(&self) -> &[&str] {
        &[]
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment>;
}
