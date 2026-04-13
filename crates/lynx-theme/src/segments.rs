use serde::{Deserialize, Serialize};

/// Universal segment fields read by the evaluator before calling render.
/// Segments never need to handle these — the evaluator filters first.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SegmentVisibility {
    /// Only show this segment in these contexts. Overrides hide_in and defaults.
    pub show_in: Option<Vec<String>>,
    /// Hide this segment in these contexts. Ignored when show_in is set.
    pub hide_in: Option<Vec<String>>,
    /// Show this segment only when condition is true. Evaluated after show_in/hide_in.
    pub show_when: Option<SegmentCondition>,
    /// Hide this segment when condition is true. Ignored when show_when is set.
    pub hide_when: Option<SegmentCondition>,
    /// Only show this segment when cwd is inside one of these directories (glob patterns).
    pub include_folders: Option<Vec<String>>,
    /// Hide this segment when cwd is inside any of these directories (glob patterns).
    /// Ignored when include_folders is set.
    pub exclude_folders: Option<Vec<String>>,
}

/// A runtime condition evaluated against `RenderContext` — no I/O, no shell.
///
/// Exactly one field should be set per condition (untagged enum: first match wins).
/// TOML example:
/// ```toml
/// [segment.username]
/// show_when = { env_set = "SSH_CONNECTION" }
///
/// [segment.git_branch]
/// show_when = { in_git_repo = true }
///
/// [segment.venv]
/// show_when = { env_matches = { VIRTUAL_ENV = "*myproject*" } }
///
/// [segment.dir]
/// show_when = { cwd_matches = "~/work/**" }
///
/// [segment.exit_code]
/// show_when = { exit_code_nonzero = true }
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SegmentCondition {
    /// Segment visible only when the named env var is set (non-empty).
    EnvSet { env_set: String },
    /// Segment visible only when all listed env vars match their glob patterns.
    EnvMatches {
        env_matches: std::collections::HashMap<String, String>,
    },
    /// `true` = only in git repos; `false` = only outside git repos.
    InGitRepo { in_git_repo: bool },
    /// Segment visible only when cwd matches the glob pattern. `~` is expanded
    /// using the `HOME` env var from the render context.
    CwdMatches { cwd_matches: String },
    /// `true` = only when last exit code is non-zero; `false` = only on zero exit.
    ExitCodeNonzero { exit_code_nonzero: bool },
    /// `true` = only when the segment's cache entry has this boolean field set to true.
    /// Used for conditional colors: `{ cache_is_true = "staged" }` matches when
    /// the segment's cache has `"staged": true`.
    CacheIsTrue { cache_is_true: String },
}

/// Shared color/style type — used by individual segment typed configs.
///
/// Supports two TOML forms:
/// - String shorthand: `color = "#7aa2f7"` (sets fg only)
/// - Full table: `color = { fg = "#7aa2f7", bg = "#1a1b26", bold = true }`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct SegmentColor {
    pub fg: Option<String>,
    #[serde(default)]
    pub bold: bool,
    pub bg: Option<String>,
}

impl<'de> serde::Deserialize<'de> for SegmentColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct ColorVisitor;

        impl<'de> de::Visitor<'de> for ColorVisitor {
            type Value = SegmentColor;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a color string or { fg, bg, bold } table")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<SegmentColor, E> {
                Ok(SegmentColor {
                    fg: Some(v.to_string()),
                    bold: false,
                    bg: None,
                })
            }

            fn visit_map<M: de::MapAccess<'de>>(self, map: M) -> Result<SegmentColor, M::Error> {
                #[derive(Deserialize)]
                struct ColorTable {
                    fg: Option<String>,
                    #[serde(default)]
                    bold: bool,
                    bg: Option<String>,
                }
                let t = ColorTable::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(SegmentColor {
                    fg: t.fg,
                    bold: t.bold,
                    bg: t.bg,
                })
            }
        }

        deserializer.deserialize_any(ColorVisitor)
    }
}

/// A conditional color override — applied when condition evaluates to true.
///
/// TOML example:
/// ```toml
/// [[segment.git_branch.color_when]]
/// condition = { cache_is_true = "staged" }
/// bg = "$git_staged_bg"
///
/// [[segment.git_branch.color_when]]
/// condition = { cache_is_true = "modified" }
/// bg = "$git_dirty_bg"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ConditionalColor {
    /// The condition to evaluate.
    pub condition: SegmentCondition,
    /// Override foreground color (falls back to base color if None).
    pub fg: Option<String>,
    /// Override background color (falls back to base color if None).
    pub bg: Option<String>,
    /// Override bold (falls back to base color if None).
    pub bold: Option<bool>,
}

/// Per-segment separator overrides — read by the renderer from `[segment.<name>]`.
/// When set, these replace the global `[separators]` glyph for this segment.
///
/// TOML example:
/// ```toml
/// [segment.shell]
/// leading_char = "\ue0b6"    # diamond cap before segment
/// trailing_char = "\ue0b4"   # diamond cap after segment
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SegmentSeparators {
    /// Character(s) rendered before this segment's content (e.g. diamond leading edge).
    pub leading_char: Option<String>,
    /// Character(s) rendered after this segment's content, replacing the normal
    /// gap separator between this segment and the next.
    pub trailing_char: Option<String>,
}

/// Shared status icon type — used by git segment config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StatusIcon {
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// The set of segment names that Lynx recognises. Used for validation.
pub const KNOWN_SEGMENTS: &[&str] = &[
    "aws_profile",
    "battery",
    "dir",
    "docker",
    "gcp",
    "git_branch",
    "git_status",
    "git_action",
    "git_ahead_behind",
    "git_sha",
    "lang_version",
    "git_stash",
    "git_time_since_commit",
    "hist_number",
    "cmd_duration",
    "context_badge",
    "kubectl_context",
    "node_version",
    "os",
    "ruby_version",
    "golang_version",
    "rust_version",
    "shell",
    "terraform",
    "text",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_color_deserialize_string_shorthand() {
        let input = "color = \"#7aa2f7\"";
        #[derive(serde::Deserialize)]
        struct W {
            color: SegmentColor,
        }
        let w: W = toml::from_str(input).unwrap();
        assert_eq!(w.color.fg, Some("#7aa2f7".to_string()));
        assert!(!w.color.bold);
        assert!(w.color.bg.is_none());
    }

    #[test]
    fn segment_color_deserialize_table() {
        let input = "[color]\nfg = \"#ff0000\"\nbg = \"#000000\"\nbold = true\n";
        #[derive(serde::Deserialize)]
        struct W {
            color: SegmentColor,
        }
        let w: W = toml::from_str(input).unwrap();
        assert_eq!(w.color.fg, Some("#ff0000".to_string()));
        assert_eq!(w.color.bg, Some("#000000".to_string()));
        assert!(w.color.bold);
    }

    #[test]
    fn segment_color_default() {
        let c = SegmentColor::default();
        assert!(c.fg.is_none());
        assert!(c.bg.is_none());
        assert!(!c.bold);
    }

    #[test]
    fn segment_visibility_default() {
        let v = SegmentVisibility::default();
        assert!(v.show_in.is_none());
        assert!(v.hide_in.is_none());
        assert!(v.show_when.is_none());
        assert!(v.hide_when.is_none());
    }

    #[test]
    fn segment_condition_env_set() {
        let input = r#"show_when = { env_set = "SSH_CONNECTION" }"#;
        #[derive(serde::Deserialize)]
        struct W {
            show_when: SegmentCondition,
        }
        let w: W = toml::from_str(input).unwrap();
        assert!(
            matches!(w.show_when, SegmentCondition::EnvSet { env_set } if env_set == "SSH_CONNECTION")
        );
    }

    #[test]
    fn segment_condition_in_git_repo() {
        let input = r#"show_when = { in_git_repo = true }"#;
        #[derive(serde::Deserialize)]
        struct W {
            show_when: SegmentCondition,
        }
        let w: W = toml::from_str(input).unwrap();
        assert!(matches!(
            w.show_when,
            SegmentCondition::InGitRepo { in_git_repo: true }
        ));
    }

    #[test]
    fn segment_condition_cwd_matches() {
        let input = r#"show_when = { cwd_matches = "~/work/**" }"#;
        #[derive(serde::Deserialize)]
        struct W {
            show_when: SegmentCondition,
        }
        let w: W = toml::from_str(input).unwrap();
        assert!(
            matches!(w.show_when, SegmentCondition::CwdMatches { cwd_matches } if cwd_matches == "~/work/**")
        );
    }

    #[test]
    fn segment_condition_exit_code_nonzero() {
        let input = r#"show_when = { exit_code_nonzero = true }"#;
        #[derive(serde::Deserialize)]
        struct W {
            show_when: SegmentCondition,
        }
        let w: W = toml::from_str(input).unwrap();
        assert!(matches!(
            w.show_when,
            SegmentCondition::ExitCodeNonzero {
                exit_code_nonzero: true
            }
        ));
    }

    #[test]
    fn segment_separators_default() {
        let s = SegmentSeparators::default();
        assert!(s.leading_char.is_none());
        assert!(s.trailing_char.is_none());
    }

    #[test]
    fn status_icon_default() {
        let s = StatusIcon::default();
        assert!(s.icon.is_none());
        assert!(s.color.is_none());
    }

    #[test]
    fn known_segments_is_not_empty() {
        assert!(!KNOWN_SEGMENTS.is_empty());
    }

    #[test]
    fn known_segments_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for seg in KNOWN_SEGMENTS {
            assert!(seen.insert(seg), "duplicate known segment: {seg}");
        }
    }

    #[test]
    fn known_segments_include_common_ones() {
        assert!(KNOWN_SEGMENTS.contains(&"dir"));
        assert!(KNOWN_SEGMENTS.contains(&"git_branch"));
        assert!(KNOWN_SEGMENTS.contains(&"prompt_char"));
        assert!(KNOWN_SEGMENTS.contains(&"time"));
    }

    #[test]
    fn conditional_color_deserialize() {
        let input =
            "[[color_when]]\ncondition = { cache_is_true = \"staged\" }\nbg = \"#00ff00\"\n";
        #[derive(serde::Deserialize)]
        struct W {
            color_when: Vec<ConditionalColor>,
        }
        let w: W = toml::from_str(input).unwrap();
        assert_eq!(w.color_when.len(), 1);
        assert_eq!(w.color_when[0].bg.as_deref(), Some("#00ff00"));
    }
}
