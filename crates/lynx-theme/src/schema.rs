use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export split-out types so `use crate::schema::X` keeps working everywhere.
pub use crate::colors::{AutoSuggestions, EzaColumns, LsColors, LsColorsEntry, SyntaxHighlight};
pub use crate::segments::{
    ConditionalColor, SegmentColor, SegmentCondition, SegmentSeparators, SegmentVisibility,
    StatusIcon, KNOWN_SEGMENTS,
};

/// Separator rendering mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SeparatorMode {
    /// One global separator style for all gaps (default — preserves existing behavior).
    #[default]
    Static,
    /// Per-gap separator colors computed from adjacent segment backgrounds.
    Adaptive,
}

/// Configuration for a single separator glyph + color.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SeparatorGlyph {
    /// The character(s) to render between segments (e.g. " " or "").
    pub char: Option<String>,
    /// Foreground color of the separator (named or hex).
    pub color: Option<String>,
    /// Background color of the separator (named or hex). Required for full powerline.
    pub bg: Option<String>,
}

/// Powerline / connector separator config — optional [separators] table in theme.
/// When absent, the renderer falls back to a single space between segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Separators {
    /// Rendering mode: static (one style for all gaps) or adaptive (per-gap colors).
    #[serde(default)]
    pub mode: SeparatorMode,
    /// Between left segments (left-to-right flow).
    #[serde(default)]
    pub left: SeparatorGlyph,
    /// Between right segments.
    #[serde(default)]
    pub right: SeparatorGlyph,
    /// Leading edge before the first left segment.
    #[serde(default)]
    pub left_edge: SeparatorGlyph,
    /// Trailing edge after the last left segment.
    #[serde(default)]
    pub right_edge: SeparatorGlyph,
}

/// Top-level theme file structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub meta: ThemeMeta,
    pub segments: SegmentLayout,
    /// Separator / connector config. Optional — defaults to space separator.
    #[serde(default)]
    pub separators: Separators,
    /// File listing color config — drives LS_COLORS and EZA_COLORS exports.
    #[serde(default)]
    pub ls_colors: LsColors,
    /// Syntax highlighting colors — drives ZSH_HIGHLIGHT_STYLES.
    #[serde(default)]
    pub syntax_highlight: SyntaxHighlight,
    /// Auto-suggestion style — drives ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE.
    #[serde(default)]
    pub autosuggestions: AutoSuggestions,
    /// Transient prompt config — shown after a command completes, replacing the
    /// full prompt. When absent, falls back to `prompt_char` segment's symbol.
    #[serde(default)]
    pub transient: Option<TransientConfig>,
    /// Debug prompt config — PROMPT4, shown for each line traced when `set -x`
    /// (xtrace) is active. When absent, zsh uses its built-in `+` prefix.
    #[serde(default)]
    pub debug_prompt: Option<DebugPromptConfig>,
    /// Per-segment config tables — raw TOML values.
    /// Each segment impl deserializes its own typed config from these.
    /// Universal fields (`show_in`, `hide_in`, `color`, `cache_ttl_ms`) are
    /// read by the evaluator before calling render.
    #[serde(default)]
    pub segment: HashMap<String, toml::Value>,
    #[serde(default)]
    pub colors: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SegmentLayout {
    #[serde(default)]
    pub left: SegmentOrder,
    #[serde(default)]
    pub right: SegmentOrder,
    /// Segments rendered on the line above the input line (multi-line prompts).
    /// When non-empty, the renderer emits a top line + newline + left segments.
    #[serde(default)]
    pub top: SegmentOrder,
    /// Segments rendered right-aligned on the top line (requires `top` to be non-empty).
    /// Uses `$COLUMNS` to compute padding so content is flush to the right edge.
    #[serde(default)]
    pub top_right: SegmentOrder,
    /// Continuation prompt config — PROMPT2 (multi-line input).
    /// When a `template` is set, it is rendered directly (with optional `fg`/`bg`)
    /// instead of the segment `order`. When only `order` is set, segments are
    /// evaluated and assembled as usual.
    #[serde(default)]
    pub continuation: ContinuationConfig,
    /// Insert a blank line before the prompt. Default: `false`.
    #[serde(default)]
    pub spacing: bool,
    /// Filler character repeated between top and top_right segments to span
    /// the full terminal width. When absent, padding is plain spaces.
    #[serde(default)]
    pub filler: Option<FillerConfig>,
}

/// Filler that stretches to fill remaining terminal width on a line.
/// Used between top and top_right segments for box-drawing prompts.
///
/// TOML example:
/// ```toml
/// [segments.filler]
/// char = "─"
/// color = "$muted"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillerConfig {
    /// The character to repeat. Default: "─".
    pub char: String,
    /// Foreground color for the filler (named or hex).
    pub color: Option<String>,
}

/// Transient prompt — the simplified prompt shown after a command runs.
/// When absent, the renderer falls back to the `prompt_char` segment symbol.
///
/// TOML example:
/// ```toml
/// [transient]
/// template = "❯ "
/// fg = "#e0f8ff"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransientConfig {
    /// The text to display as the transient prompt.
    pub template: String,
    /// Foreground color (named or hex).
    pub fg: Option<String>,
    /// Background color (named or hex).
    pub bg: Option<String>,
}

/// Continuation prompt config — drives PROMPT2 (multi-line input continuation).
///
/// TOML examples:
/// ```toml
/// # Template mode — renders a fixed string (no segments evaluated)
/// [segments.continuation]
/// template = "… "
/// fg = "#6272a4"
///
/// # Segment mode — reuses existing segment infrastructure
/// [segments.continuation]
/// order = ["prompt_char"]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContinuationConfig {
    /// Segment names to evaluate (used when `template` is absent).
    #[serde(default)]
    pub order: Vec<String>,
    /// Fixed text to render as PROMPT2. When set, `order` is ignored.
    pub template: Option<String>,
    /// Foreground color for the template (named or hex). Ignored when `template` is absent.
    pub fg: Option<String>,
    /// Background color for the template (named or hex). Ignored when `template` is absent.
    pub bg: Option<String>,
}

/// Debug prompt config — drives PROMPT4 (`set -x` xtrace output prefix).
/// When absent, zsh uses its default `+ ` prefix.
///
/// TOML example:
/// ```toml
/// [debug_prompt]
/// template = "+ "
/// fg = "#565f89"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugPromptConfig {
    /// The text to display as the debug/trace prompt.
    pub template: String,
    /// Foreground color (named or hex).
    pub fg: Option<String>,
    /// Background color (named or hex).
    pub bg: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SegmentOrder {
    #[serde(default)]
    pub order: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_mode_default_is_static() {
        let mode = SeparatorMode::default();
        assert_eq!(mode, SeparatorMode::Static);
    }

    #[test]
    fn separator_glyph_default() {
        let g = SeparatorGlyph::default();
        assert!(g.char.is_none());
        assert!(g.color.is_none());
        assert!(g.bg.is_none());
    }

    #[test]
    fn separators_default() {
        let s = Separators::default();
        assert_eq!(s.mode, SeparatorMode::Static);
    }

    #[test]
    fn segment_layout_default() {
        let l = SegmentLayout::default();
        assert!(l.left.order.is_empty());
        assert!(l.right.order.is_empty());
        assert!(l.top.order.is_empty());
        assert!(l.top_right.order.is_empty());
        assert!(!l.spacing);
        assert!(l.filler.is_none());
    }

    #[test]
    fn theme_meta_deserialize() {
        let toml = r#"
            name = "test"
            description = "A test theme"
            author = "me"
        "#;
        let meta: ThemeMeta = toml::from_str(toml).unwrap();
        assert_eq!(meta.name, "test");
        assert_eq!(meta.description, "A test theme");
        assert_eq!(meta.author, "me");
    }

    #[test]
    fn segment_order_default_empty() {
        let so = SegmentOrder::default();
        assert!(so.order.is_empty());
    }

    #[test]
    fn transient_config_deserialize() {
        let toml = "template = \"> \"\nfg = \"#fff\"\n";
        let tc: TransientConfig = toml::from_str(toml).unwrap();
        assert_eq!(tc.template, "> ");
        assert_eq!(tc.fg.as_deref(), Some("#fff"));
        assert!(tc.bg.is_none());
    }

    #[test]
    fn filler_config_deserialize() {
        let toml = "char = \"-\"\ncolor = \"#888\"\n";
        let fc: FillerConfig = toml::from_str(toml).unwrap();
        assert_eq!(fc.char, "-");
        assert_eq!(fc.color.as_deref(), Some("#888"));
    }
}
