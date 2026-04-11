use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a single separator glyph + color.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SeparatorGlyph {
    /// The character(s) to render between segments (e.g. " " or "").
    pub char: Option<String>,
    /// Foreground color of the separator (named or hex).
    pub color: Option<String>,
}

/// Powerline / connector separator config — optional [separators] table in theme.
/// When absent, the renderer falls back to a single space between segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Separators {
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

/// One entry in the `[ls_colors]` table — colors for a single file-type category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsColorsEntry {
    pub fg: Option<String>,
    pub bg: Option<String>,
    #[serde(default)]
    pub bold: bool,
}

/// The `[ls_colors]` table — semantic mapping from file-type categories to colors.
/// Absent fields default to no override (terminal/distro default applies).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsColors {
    pub dir: Option<LsColorsEntry>,
    pub symlink: Option<LsColorsEntry>,
    pub executable: Option<LsColorsEntry>,
    pub archive: Option<LsColorsEntry>,
    pub image: Option<LsColorsEntry>,
    pub audio: Option<LsColorsEntry>,
    pub broken: Option<LsColorsEntry>,
    pub other_writable: Option<LsColorsEntry>,
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
    /// Per-segment config tables — raw TOML values.
    /// Each segment impl deserializes its own typed config from these.
    /// Universal fields (`show_in`, `hide_in`, `color`, `cache_ttl_ms`) are
    /// read by the evaluator before calling render.
    #[serde(default)]
    pub segment: HashMap<String, toml::Value>,
    #[serde(default)]
    pub colors: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Segments rendered as PROMPT2 (continuation prompt for multi-line input).
    #[serde(default)]
    pub continuation: SegmentOrder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SegmentOrder {
    #[serde(default)]
    pub order: Vec<String>,
}

/// Universal segment fields read by the evaluator before calling render.
/// Segments never need to handle these — the evaluator filters first.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SegmentVisibility {
    /// Only show this segment in these contexts. Overrides hide_in and defaults.
    pub show_in: Option<Vec<String>>,
    /// Hide this segment in these contexts. Ignored when show_in is set.
    pub hide_in: Option<Vec<String>>,
}

/// Shared color/style type — used by individual segment typed configs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SegmentColor {
    pub fg: Option<String>,
    #[serde(default)]
    pub bold: bool,
    pub bg: Option<String>,
}

/// Shared status icon type — used by git segment config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StatusIcon {
    pub icon: Option<String>,
    pub color: Option<String>,
}

impl LsColors {
    /// Build the value string for `export LS_COLORS=...`.
    ///
    /// Format: `di=<sgr>:ln=<sgr>:ex=<sgr>:...` (colon-separated type=sgr pairs).
    /// Extension-based entries (`*.tar`, `*.jpg`, etc.) are appended for archive,
    /// image, and audio categories.
    ///
    /// Returns `None` when the `[ls_colors]` table is entirely absent (all fields None).
    pub fn to_ls_colors_string(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();

        if let Some(e) = &self.dir {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("di={sgr}"));
            }
        }
        if let Some(e) = &self.symlink {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("ln={sgr}"));
            }
        }
        if let Some(e) = &self.executable {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("ex={sgr}"));
            }
        }
        if let Some(e) = &self.broken {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("or={sgr}"));
            }
        }
        if let Some(e) = &self.other_writable {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("ow={sgr}"));
            }
        }
        if let Some(e) = &self.archive {
            if let Some(sgr) = entry_sgr(e) {
                for ext in &[
                    "tar", "gz", "bz2", "xz", "zip", "7z", "rar", "tgz", "zst", "lz4",
                ] {
                    parts.push(format!("*.{ext}={sgr}"));
                }
            }
        }
        if let Some(e) = &self.image {
            if let Some(sgr) = entry_sgr(e) {
                for ext in &[
                    "jpg", "jpeg", "png", "gif", "bmp", "svg", "webp", "ico", "tiff", "tif",
                ] {
                    parts.push(format!("*.{ext}={sgr}"));
                }
            }
        }
        if let Some(e) = &self.audio {
            if let Some(sgr) = entry_sgr(e) {
                for ext in &["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus", "wma"] {
                    parts.push(format!("*.{ext}={sgr}"));
                }
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(":"))
        }
    }

    /// Build the value string for `export EZA_COLORS=...`.
    ///
    /// EZA reads `LS_COLORS` first, then `EZA_COLORS` as overrides. The format
    /// is identical to `LS_COLORS`. We emit the same entries here — users can
    /// layer additional EZA-specific keys on top in their own config.
    pub fn to_eza_colors_string(&self) -> Option<String> {
        self.to_ls_colors_string()
    }
}

/// Convert an `LsColorsEntry` to an ANSI SGR parameter string (e.g. `"1;34"`).
/// Returns `None` if the entry has no color information.
fn entry_sgr(e: &LsColorsEntry) -> Option<String> {
    let mut codes: Vec<String> = Vec::new();

    if e.bold {
        codes.push("1".to_string());
    }
    if let Some(fg) = &e.fg {
        if let Some(sgr) = color_to_fg_sgr(fg) {
            codes.push(sgr);
        }
    }
    if let Some(bg) = &e.bg {
        if let Some(sgr) = color_to_bg_sgr(bg) {
            codes.push(sgr);
        }
    }

    if codes.is_empty() {
        None
    } else {
        Some(codes.join(";"))
    }
}

/// Convert a color string (named or hex) to an ANSI 256-color fg SGR parameter.
/// Returns `None` for unknown colors.
fn color_to_fg_sgr(color: &str) -> Option<String> {
    let (r, g, b) = resolve_color_rgb(color)?;
    let idx = rgb_to_256(r, g, b);
    Some(format!("38;5;{idx}"))
}

/// Convert a color string (named or hex) to an ANSI 256-color bg SGR parameter.
fn color_to_bg_sgr(color: &str) -> Option<String> {
    let (r, g, b) = resolve_color_rgb(color)?;
    let idx = rgb_to_256(r, g, b);
    Some(format!("48;5;{idx}"))
}

/// Resolve a color string to (r, g, b). Handles hex (#rrggbb) and named colors.
fn resolve_color_rgb(color: &str) -> Option<(u8, u8, u8)> {
    if color.starts_with('#') {
        parse_hex_rgb(color)
    } else {
        crate::color::named_to_rgb(color)
    }
}

fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let s = hex.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Nearest xterm-256 color cube index for an RGB value (mirrors color.rs logic).
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    let ri = (r as u32 * 5 / 255) as u8;
    let gi = (g as u32 * 5 / 255) as u8;
    let bi = (b as u32 * 5 / 255) as u8;
    16 + 36 * ri + 6 * gi + bi
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blue_dir_entry() -> LsColorsEntry {
        LsColorsEntry {
            fg: Some("blue".to_string()),
            bg: None,
            bold: true,
        }
    }

    #[test]
    fn empty_ls_colors_returns_none() {
        let lsc = LsColors::default();
        assert!(lsc.to_ls_colors_string().is_none());
    }

    #[test]
    fn dir_entry_produces_di_key() {
        let lsc = LsColors {
            dir: Some(blue_dir_entry()),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.starts_with("di="), "expected di= prefix, got: {s}");
        // bold + 256-color fg code
        assert!(s.contains("1;38;5;"), "expected bold+256-color code in: {s}");
    }

    #[test]
    fn archive_entry_expands_to_extensions() {
        let lsc = LsColors {
            archive: Some(LsColorsEntry {
                fg: Some("red".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.contains("*.tar="), "expected *.tar in: {s}");
        assert!(s.contains("*.zip="), "expected *.zip in: {s}");
        assert!(s.contains("*.gz="), "expected *.gz in: {s}");
    }

    #[test]
    fn image_entry_expands_to_extensions() {
        let lsc = LsColors {
            image: Some(LsColorsEntry {
                fg: Some("magenta".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.contains("*.jpg="), "expected *.jpg in: {s}");
        assert!(s.contains("*.png="), "expected *.png in: {s}");
    }

    #[test]
    fn hex_color_is_converted_to_256() {
        let lsc = LsColors {
            dir: Some(LsColorsEntry {
                fg: Some("#7aa2f7".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.contains("38;5;"), "expected 256-color code in: {s}");
    }

    #[test]
    fn bg_color_produces_48_code() {
        let lsc = LsColors {
            dir: Some(LsColorsEntry {
                fg: Some("blue".to_string()),
                bg: Some("green".to_string()),
                bold: false,
            }),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.contains("48;5;"), "expected bg 256-color code in: {s}");
    }

    #[test]
    fn eza_colors_matches_ls_colors() {
        let lsc = LsColors {
            dir: Some(blue_dir_entry()),
            ..Default::default()
        };
        assert_eq!(lsc.to_ls_colors_string(), lsc.to_eza_colors_string());
    }

    #[test]
    fn inline_toml_ls_colors_parses() {
        // Minimal reproduction to confirm TOML parsing works for ls_colors.
        let toml = r#"
[meta]
name = "test"

[segments.left]
order = []

[ls_colors]
dir = { fg = "blue", bold = true }
"#;
        let theme: super::Theme = toml::from_str(toml).expect("should parse");
        assert!(theme.ls_colors.dir.is_some(), "dir should be Some after parse");
        let s = theme.ls_colors.to_ls_colors_string().unwrap();
        assert!(s.contains("di="), "expected di= in: {s}");
    }

    #[test]
    fn default_theme_ls_colors_is_non_empty() {
        let theme = crate::loader::load("default").unwrap();
        assert!(
            theme.ls_colors.to_ls_colors_string().is_some(),
            "default theme should have non-empty ls_colors; dir={:?}", theme.ls_colors.dir
        );
    }

    #[test]
    fn minimal_theme_ls_colors_is_non_empty() {
        let theme = crate::loader::load("minimal").unwrap();
        assert!(
            theme.ls_colors.to_ls_colors_string().is_some(),
            "minimal theme should have non-empty ls_colors"
        );
    }
}

/// The set of segment names that Lynx recognises. Used for validation.
pub const KNOWN_SEGMENTS: &[&str] = &[
    "dir",
    "git_branch",
    "git_status",
    "git_action",
    "git_ahead_behind",
    "git_stash",
    "cmd_duration",
    "context_badge",
    "kubectl_context",
    "profile_badge",
    "username",
    "hostname",
    "ssh_indicator",
    "venv",
    "conda_env",
    "task_status",
    "exit_code",
    "background_jobs",
    "vi_mode",
    "time",
    "newline",
    "prompt_char",
];
