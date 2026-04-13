use lynx_theme::{
    schema::{ConditionalColor, SegmentColor, SegmentSeparators, SeparatorMode, Separators},
    terminal::{capability, TermCapability},
};

use crate::color_apply::apply_color_zsh;
use crate::segment::RenderedSegment;

/// Resolve the bg color string for a segment from theme config.
pub(crate) fn resolve_seg_bg(
    seg: &RenderedSegment,
    theme: &lynx_theme::schema::Theme,
) -> Option<String> {
    seg.cache_key
        .as_deref()
        .and_then(|name| theme.segment.get(name))
        .and_then(|sc| sc.get("color"))
        .and_then(|c| c.get("bg"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Render the gap separator between segment `i` and segment `i+1`.
/// Uses the provided `gap_char` (which may be a per-segment trailing_char or the global separator).
fn render_gap(
    gap_char: &str,
    i: usize,
    segs: &[RenderedSegment],
    theme: &lynx_theme::schema::Theme,
    sep: &Separators,
    glyph: &lynx_theme::schema::SeparatorGlyph,
    cap: TermCapability,
) -> String {
    if cap == TermCapability::None {
        return gap_char.to_string();
    }

    match sep.mode {
        SeparatorMode::Static => {
            if glyph.color.is_some() || glyph.bg.is_some() {
                let color = SegmentColor {
                    fg: glyph.color.clone(),
                    bold: false,
                    bg: glyph.bg.clone(),
                };
                apply_color_zsh(gap_char, &color, cap)
            } else {
                gap_char.to_string()
            }
        }
        SeparatorMode::Adaptive => {
            let prev_bg = resolve_seg_bg(&segs[i], theme);
            let next_bg = resolve_seg_bg(&segs[i + 1], theme);
            if prev_bg.is_some() {
                let color = SegmentColor {
                    fg: prev_bg,
                    bg: next_bg,
                    bold: false,
                };
                apply_color_zsh(gap_char, &color, cap)
            } else if let Some(ref col) = glyph.color {
                let color = SegmentColor {
                    fg: Some(col.clone()),
                    bold: false,
                    bg: None,
                };
                apply_color_zsh(gap_char, &color, cap)
            } else {
                gap_char.to_string()
            }
        }
    }
}

/// Apply min_width / max_width constraints from segment theme config.
/// Pads with spaces (min) or truncates with ellipsis (max).
fn apply_width_constraints(
    text: &str,
    seg: &RenderedSegment,
    theme: &lynx_theme::schema::Theme,
) -> String {
    let config = seg
        .cache_key
        .as_deref()
        .and_then(|name| theme.segment.get(name));

    let min_width = config
        .and_then(|c| c.get("min_width"))
        .and_then(|v| v.as_integer())
        .map(|v| v as usize);
    let max_width = config
        .and_then(|c| c.get("max_width"))
        .and_then(|v| v.as_integer())
        .map(|v| v as usize);

    let mut result = text.to_string();

    if let Some(max) = max_width {
        let char_count = result.chars().count();
        if char_count > max && max >= 1 {
            result = result.chars().take(max - 1).collect::<String>() + "…";
        }
    }

    if let Some(min) = min_width {
        let char_count = result.chars().count();
        if char_count < min {
            result.push_str(&" ".repeat(min - char_count));
        }
    }

    result
}

/// Resolve per-segment separator overrides from theme config.
fn resolve_seg_separators(
    seg: &RenderedSegment,
    theme: &lynx_theme::schema::Theme,
) -> SegmentSeparators {
    seg.cache_key
        .as_deref()
        .and_then(|name| theme.segment.get(name))
        .and_then(|sc| {
            let leading = sc
                .get("leading_char")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let trailing = sc
                .get("trailing_char")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if leading.is_some() || trailing.is_some() {
                Some(SegmentSeparators {
                    leading_char: leading,
                    trailing_char: trailing,
                })
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Like `assemble` but without the trailing space (used for top_right so padding
/// is controlled externally).
pub(crate) fn assemble_no_trail(
    segs: &[RenderedSegment],
    theme: &lynx_theme::schema::Theme,
    sep: &Separators,
    is_left: bool,
    ctx: Option<&crate::segment::RenderContext>,
) -> String {
    let assembled = assemble(segs, theme, sep, is_left, ctx);
    // assemble() always appends a trailing space; strip it for right-side content.
    assembled
        .strip_suffix(' ')
        .unwrap_or(&assembled)
        .to_string()
}

/// Assemble a prompt string from segments, applying theme colors and separators.
///
/// ANSI escape sequences are wrapped in zsh `%{...%}` so zsh does not count
/// them as visible characters when computing line length. Without this, the
/// cursor position is miscalculated and line editing breaks.
///
/// `is_left` controls which separator (left/right) is used between segments.
pub(crate) fn assemble(
    segs: &[RenderedSegment],
    theme: &lynx_theme::schema::Theme,
    sep: &Separators,
    is_left: bool,
    ctx: Option<&crate::segment::RenderContext>,
) -> String {
    if segs.is_empty() {
        return if is_left {
            "$ ".to_string()
        } else {
            String::new()
        };
    }

    let cap = capability();

    // Resolve color configs and render colored text for each segment.
    // If ctx is provided and the segment has color_when, evaluate conditions.
    let color_cfgs: Vec<Option<SegmentColor>> = segs
        .iter()
        .map(|seg| {
            let seg_config = seg
                .cache_key
                .as_deref()
                .and_then(|name| theme.segment.get(name));

            let base_color: Option<SegmentColor> = seg_config
                .and_then(|sc| sc.get("color"))
                .and_then(|c| c.clone().try_into().ok());

            // Check for color_when conditional overrides.
            if let (Some(base), Some(render_ctx)) = (&base_color, ctx) {
                let color_when: Vec<ConditionalColor> = seg_config
                    .and_then(|sc| sc.get("color_when"))
                    .and_then(|cw| cw.clone().try_into().ok())
                    .unwrap_or_default();
                if !color_when.is_empty() {
                    return Some(crate::evaluator::resolve_conditional_color(
                        base,
                        &color_when,
                        render_ctx,
                    ));
                }
            }

            base_color
        })
        .collect();

    let parts: Vec<String> = segs
        .iter()
        .zip(color_cfgs.iter())
        .map(|(seg, color_cfg)| {
            // Apply width constraints (min_width pads, max_width truncates).
            let text = apply_width_constraints(&seg.text, seg, theme);

            if cap != TermCapability::None {
                if let Some(ref color) = color_cfg {
                    apply_color_zsh(&text, color, cap)
                } else {
                    text
                }
            } else {
                text
            }
        })
        .collect();

    let glyph = if is_left { &sep.left } else { &sep.right };
    let sep_str = glyph.char.as_deref().unwrap_or(" ");

    // Render left_edge before first segment if configured.
    // In adaptive mode, auto-derive edge fg from first segment's bg when no
    // explicit edge color is set — this creates a smooth "fade-in" cap.
    let edge_glyph = if is_left {
        &sep.left_edge
    } else {
        &sep.right_edge
    };
    let edge_str = edge_glyph.char.as_deref().unwrap_or("");
    let edge_rendered = if !edge_str.is_empty() && cap != TermCapability::None {
        let edge_fg = edge_glyph.color.clone().or_else(|| {
            if sep.mode == SeparatorMode::Adaptive {
                resolve_seg_bg(&segs[0], theme)
            } else {
                None
            }
        });
        if let Some(col) = edge_fg {
            let color = SegmentColor {
                fg: Some(col),
                bold: false,
                bg: edge_glyph.bg.clone(),
            };
            apply_color_zsh(edge_str, &color, cap)
        } else {
            edge_str.to_string()
        }
    } else {
        edge_str.to_string()
    };

    // Resolve per-segment separator overrides for each segment.
    let seg_seps: Vec<SegmentSeparators> = segs
        .iter()
        .map(|seg| resolve_seg_separators(seg, theme))
        .collect();

    // Build the joined string, interleaving per-segment leading/trailing chars.
    let mut joined = String::new();
    for (i, part) in parts.iter().enumerate() {
        // Leading char: rendered before this segment's content.
        if let Some(ref lead) = seg_seps[i].leading_char {
            if cap != TermCapability::None {
                // In adaptive mode, color the leading char: fg = this seg's bg.
                let fg = if sep.mode == SeparatorMode::Adaptive {
                    resolve_seg_bg(&segs[i], theme)
                } else {
                    glyph.color.clone()
                };
                if let Some(col) = fg {
                    let color = SegmentColor {
                        fg: Some(col),
                        bg: None,
                        bold: false,
                    };
                    joined.push_str(&apply_color_zsh(lead, &color, cap));
                } else {
                    joined.push_str(lead);
                }
            } else {
                joined.push_str(lead);
            }
        }

        // Segment content (already colored).
        joined.push_str(part);

        // Gap between this segment and the next (or tail after last segment).
        if i + 1 < parts.len() {
            // Use trailing_char from current segment if set, else global separator.
            let gap_char = seg_seps[i].trailing_char.as_deref().unwrap_or(sep_str);
            joined.push_str(&render_gap(gap_char, i, segs, theme, sep, glyph, cap));
        } else {
            // Last segment — emit trailing_char or adaptive tail arrow.
            if let Some(ref trail) = seg_seps[i].trailing_char {
                if cap != TermCapability::None {
                    let fg = resolve_seg_bg(&segs[i], theme);
                    if let Some(col) = fg {
                        let color = SegmentColor {
                            fg: Some(col),
                            bg: None,
                            bold: false,
                        };
                        joined.push_str(&apply_color_zsh(trail, &color, cap));
                    } else {
                        joined.push_str(trail);
                    }
                } else {
                    joined.push_str(trail);
                }
            } else if sep.mode == SeparatorMode::Adaptive {
                // Default tail arrow for adaptive mode.
                if let Some(last_bg) = segs.last().and_then(|s| resolve_seg_bg(s, theme)) {
                    if cap != TermCapability::None {
                        let color = SegmentColor {
                            fg: Some(last_bg),
                            bg: None,
                            bold: false,
                        };
                        joined.push_str(&apply_color_zsh(sep_str, &color, cap));
                    }
                }
            }
        }
    }

    if edge_rendered.is_empty() {
        format!("{joined} ")
    } else {
        format!("{edge_rendered}{joined} ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_theme::schema::SeparatorMode;
    use lynx_theme::{
        parse_and_validate,
        terminal::{clear_capability_override, override_capability},
    };

    struct CapabilityGuard;

    impl Drop for CapabilityGuard {
        fn drop(&mut self) {
            clear_capability_override();
        }
    }

    fn set_capability(cap: TermCapability) -> CapabilityGuard {
        override_capability(cap);
        CapabilityGuard
    }

    fn load_default() -> lynx_theme::schema::Theme {
        parse_and_validate(include_str!("../../../themes/default.toml"), "default").unwrap()
    }

    #[test]
    fn colored_segment_contains_zsh_wrappers() {
        let _cap = set_capability(TermCapability::Ansi256);

        let theme = load_default();
        // dir segment has color = { fg = "blue", bold = true } in default theme
        let segs = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let result = assemble(&segs, &theme, &theme.separators, true, None);

        assert!(
            result.contains("%{") && result.contains("%}"),
            "expected zsh %{{...%}} wrappers in: {result:?}"
        );
    }

    #[test]
    fn no_color_terminal_produces_plain_text() {
        let _cap = set_capability(TermCapability::None);

        let theme = load_default();
        let segs = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let result = assemble(&segs, &theme, &theme.separators, true, None);

        assert!(
            !result.contains("\x1b["),
            "expected no ANSI escapes in: {result:?}"
        );
    }

    #[test]
    fn adaptive_separator_uses_prev_bg_as_fg() {
        let _cap = set_capability(TermCapability::TrueColor);

        // Build a theme where segments have bg colors.
        let mut theme = load_default();
        // Set dir bg=blue, git_branch bg=green via segment config.
        let dir_color = toml::Value::try_from(toml::toml! {
            color = { fg = "white", bg = "blue" }
        })
        .unwrap();
        let git_color = toml::Value::try_from(toml::toml! {
            color = { fg = "black", bg = "green" }
        })
        .unwrap();
        theme.segment.insert("dir".to_string(), dir_color);
        theme.segment.insert("git_branch".to_string(), git_color);

        let mut sep = Separators {
            mode: SeparatorMode::Adaptive,
            ..Default::default()
        };
        sep.left.char = Some("\u{e0b0}".to_string()); //

        let segs = vec![
            RenderedSegment::new("~/code").with_cache_key("dir"),
            RenderedSegment::new("main").with_cache_key("git_branch"),
        ];

        let result = assemble(&segs, &theme, &sep, true, None);
        // Separator between dir→git_branch should have fg=blue (dir's bg).
        // blue = (122, 162, 247) → 38;2;122;162;247
        assert!(
            result.contains("38;2;122;162;247"),
            "separator fg should be dir's bg (blue): {result:?}"
        );
        // Separator bg should be green (git_branch's bg).
        // green = (158, 206, 106) → 48;2;158;206;106
        assert!(
            result.contains("48;2;158;206;106"),
            "separator bg should be git_branch's bg (green): {result:?}"
        );
    }

    #[test]
    fn adaptive_tail_arrow_emitted() {
        let _cap = set_capability(TermCapability::TrueColor);

        let mut theme = load_default();
        let dir_color = toml::Value::try_from(toml::toml! {
            color = { fg = "white", bg = "blue" }
        })
        .unwrap();
        theme.segment.insert("dir".to_string(), dir_color);

        let mut sep = Separators {
            mode: SeparatorMode::Adaptive,
            ..Default::default()
        };
        sep.left.char = Some("\u{e0b0}".to_string());

        let segs = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let result = assemble(&segs, &theme, &sep, true, None);
        // Tail arrow: separator after last segment with fg=blue, no bg.
        // Count occurrences of the separator char — should appear once (tail arrow).
        let sep_count = result.matches('\u{e0b0}').count();
        assert!(sep_count >= 1, "expected tail arrow separator: {result:?}");
    }

    #[test]
    fn static_mode_unchanged_from_default() {
        let _cap = set_capability(TermCapability::None);

        let theme = load_default();
        let segs = vec![
            RenderedSegment::new("a").with_cache_key("dir"),
            RenderedSegment::new("b").with_cache_key("git_branch"),
        ];

        // Default separators have mode=Static.
        let result_static = assemble(&segs, &theme, &theme.separators, true, None);

        // Explicitly set Static.
        let mut sep = theme.separators.clone();
        sep.mode = SeparatorMode::Static;
        let result_explicit = assemble(&segs, &theme, &sep, true, None);

        assert_eq!(
            result_static, result_explicit,
            "Static mode must be byte-identical to default"
        );
    }

    #[test]
    fn custom_separator_char_is_used() {
        let _cap = set_capability(TermCapability::None);
        let theme = load_default();
        let segs = vec![
            RenderedSegment::new("a").with_cache_key("dir"),
            RenderedSegment::new("b").with_cache_key("git_branch"),
        ];
        let mut sep = Separators::default();
        sep.left.char = Some("|".to_string());
        let result = assemble(&segs, &theme, &sep, true, None);
        assert!(result.contains('|'), "expected | separator in: {result:?}");
    }

    #[test]
    fn per_segment_leading_char() {
        let _cap = set_capability(TermCapability::None);
        let mut theme = load_default();
        // Set leading_char on dir segment.
        let dir_cfg = toml::Value::try_from(toml::toml! {
            leading_char = "\u{e0b6}"
        })
        .unwrap();
        theme.segment.insert("dir".to_string(), dir_cfg);

        let segs = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let result = assemble(&segs, &theme, &theme.separators, true, None);
        assert!(
            result.contains('\u{e0b6}'),
            "expected leading char: {result:?}"
        );
        assert!(
            result.contains("~/code"),
            "expected segment content: {result:?}"
        );
        // Leading char should appear before the content.
        let lead_pos = result.find('\u{e0b6}').unwrap();
        let content_pos = result.find("~/code").unwrap();
        assert!(
            lead_pos < content_pos,
            "leading char should precede content: {result:?}"
        );
    }

    #[test]
    fn per_segment_trailing_char_replaces_gap() {
        let _cap = set_capability(TermCapability::None);
        let mut theme = load_default();
        // Set trailing_char on dir segment — should replace the gap between dir and git.
        let dir_cfg = toml::Value::try_from(toml::toml! {
            trailing_char = "\u{e0b4}"
        })
        .unwrap();
        theme.segment.insert("dir".to_string(), dir_cfg);

        let mut sep = Separators::default();
        sep.left.char = Some("|".to_string()); // Global separator.

        let segs = vec![
            RenderedSegment::new("~/code").with_cache_key("dir"),
            RenderedSegment::new("main").with_cache_key("git_branch"),
        ];
        let result = assemble(&segs, &theme, &sep, true, None);
        // The gap between dir and git_branch should use trailing_char, not "|".
        assert!(
            result.contains('\u{e0b4}'),
            "expected trailing char in gap: {result:?}"
        );
        // The global "|" should NOT appear between these two segments.
        // (It might not appear at all since there's only one gap.)
        let between = &result[result.find("~/code").unwrap()..result.find("main").unwrap()];
        assert!(
            !between.contains('|'),
            "global separator should not appear in gap with trailing_char: {between:?}"
        );
    }

    #[test]
    fn per_segment_trailing_char_on_last_segment() {
        let _cap = set_capability(TermCapability::None);
        let mut theme = load_default();
        let dir_cfg = toml::Value::try_from(toml::toml! {
            trailing_char = "\u{e0b4}"
        })
        .unwrap();
        theme.segment.insert("dir".to_string(), dir_cfg);

        let segs = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let result = assemble(&segs, &theme, &theme.separators, true, None);
        assert!(
            result.contains('\u{e0b4}'),
            "expected trailing char after last segment: {result:?}"
        );
    }

    #[test]
    fn diamond_segment_mixed_with_powerline() {
        let _cap = set_capability(TermCapability::None);
        let mut theme = load_default();
        // Shell segment gets diamond caps.
        let shell_cfg = toml::Value::try_from(toml::toml! {
            leading_char = "\u{e0b6}"
            trailing_char = "\u{e0b4}"
        })
        .unwrap();
        theme.segment.insert("shell".to_string(), shell_cfg);
        // Dir segment uses default (global separator).

        let mut sep = Separators::default();
        sep.left.char = Some("\u{e0b0}".to_string()); // Powerline arrow.

        let segs = vec![
            RenderedSegment::new("zsh").with_cache_key("shell"),
            RenderedSegment::new("~/code").with_cache_key("dir"),
        ];
        let result = assemble(&segs, &theme, &sep, true, None);
        // Shell should have diamond caps.
        assert!(
            result.contains('\u{e0b6}'),
            "expected leading diamond: {result:?}"
        );
        assert!(
            result.contains('\u{e0b4}'),
            "expected trailing diamond: {result:?}"
        );
        // Dir should NOT have diamond caps.
        assert!(
            result.contains("~/code"),
            "expected dir content: {result:?}"
        );
    }

    #[test]
    fn no_per_segment_overrides_matches_original_behavior() {
        // Verify that when no segments have leading_char/trailing_char,
        // the output is identical to the old behavior.
        let _cap = set_capability(TermCapability::None);
        let theme = load_default();
        let segs = vec![
            RenderedSegment::new("a").with_cache_key("dir"),
            RenderedSegment::new("b").with_cache_key("git_branch"),
        ];
        let result = assemble(&segs, &theme, &theme.separators, true, None);
        // With no separator config, segments are space-separated.
        assert_eq!(
            result, "a b ",
            "default behavior should be space-separated: {result:?}"
        );
    }
}
