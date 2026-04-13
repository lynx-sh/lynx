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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Substitute `$variable` references in a format template string.
///
/// Each entry in `vars` is a `(name, value)` pair. Variables are matched by
/// the longest name first (so `$branch_name` beats `$branch`). Unknown
/// variables expand to an empty string. A literal `$$` produces a single `$`.
///
/// # Example
/// ```
/// use lynx_prompt::segment::apply_format;
/// let out = apply_format("[$branch]($style) $status", &[
///     ("branch", "main"),
///     ("style", "bold"),
///     ("status", "!"),
/// ]);
/// assert_eq!(out, "[main](bold) !");
/// ```
pub fn apply_format(template: &str, vars: &[(&str, &str)]) -> String {
    // Sort vars by name length descending so longer names match first.
    let mut sorted: Vec<(&str, &str)> = vars.to_vec();
    sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let mut result = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '$' {
            // Escaped dollar sign.
            if i + 1 < chars.len() && chars[i + 1] == '$' {
                result.push('$');
                i += 2;
                continue;
            }
            // Collect the variable name (alphanumeric + underscore).
            let name_start = i + 1;
            let mut j = name_start;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let name: String = chars[name_start..j].iter().collect();
            if name.is_empty() {
                result.push('$');
                i += 1;
            } else {
                // Find the first matching var (sorted by name length desc).
                let value = sorted
                    .iter()
                    .find(|(k, _)| *k == name.as_str())
                    .map(|(_, v)| *v)
                    .unwrap_or("");
                result.push_str(value);
                i = j;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
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

#[cfg(test)]
mod tests {
    use super::apply_format;

    #[test]
    fn basic_substitution() {
        let out = apply_format("$icon$branch", &[("icon", " "), ("branch", "main")]);
        assert_eq!(out, " main");
    }

    #[test]
    fn bracket_template_like_starship() {
        let out = apply_format(
            "[$branch]($style)",
            &[("branch", "main"), ("style", "bold")],
        );
        assert_eq!(out, "[main](bold)");
    }

    #[test]
    fn unknown_var_expands_to_empty() {
        let out = apply_format("$known $unknown", &[("known", "yes")]);
        assert_eq!(out, "yes ");
    }

    #[test]
    fn escaped_dollar_produces_literal() {
        let out = apply_format("$$branch", &[("branch", "main")]);
        assert_eq!(out, "$branch");
    }

    #[test]
    fn empty_var_value_removed() {
        let out = apply_format("$a$b$c", &[("a", "x"), ("b", ""), ("c", "z")]);
        assert_eq!(out, "xz");
    }

    #[test]
    fn no_vars_passes_through() {
        let out = apply_format("hello world", &[]);
        assert_eq!(out, "hello world");
    }
}
