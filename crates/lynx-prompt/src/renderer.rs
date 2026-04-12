use lynx_theme::{
    schema::{ConditionalColor, SegmentColor, SegmentSeparators, SeparatorMode, Separators, Theme},
    terminal::{capability, TermCapability},
};

use crate::color_apply::apply_color_zsh;
use crate::segment::RenderedSegment;

/// Assemble PROMPT and RPROMPT shell assignments from rendered segments.
///
/// When `top` is non-empty the output is a two-line prompt:
///   PROMPT="<top line>"$'\n'"<left line> "
///
/// When `top_right` is non-empty, its content is right-aligned on the top line
/// using `columns` to compute padding (falls back to no padding if unknown).
///
/// When `continuation` is non-empty, PROMPT2 is also emitted.
pub fn render_prompt(
    left: &[RenderedSegment],
    right: &[RenderedSegment],
    top: &[RenderedSegment],
    top_right: &[RenderedSegment],
    continuation: &[RenderedSegment],
    theme: &Theme,
    columns: Option<u32>,
    ctx: Option<&crate::segment::RenderContext>,
) -> String {
    let sep = &theme.separators;

    // Build the input-line part of PROMPT (left segments).
    // When a top line exists, the bottom (input) line should render plain —
    // no powerline edge glyphs, no adaptive separators.
    let left_str = if !top.is_empty() {
        let mut plain_sep = Separators::default();
        plain_sep.mode = SeparatorMode::Static;
        assemble(left, theme, &plain_sep, true, ctx)
    } else {
        assemble(left, theme, sep, true, ctx)
    };
    let rprompt = assemble(right, theme, sep, false, ctx);

    // Optional blank line before the prompt for visual breathing room.
    let spacer = if theme.segments.spacing { "$'\\n'" } else { "" };

    let mut out = if top.is_empty() {
        format!("PROMPT={spacer}\"{left_str}\"\nRPROMPT=\"{rprompt}\"\n")
    } else {
        let top_str = assemble(top, theme, sep, true, ctx);
        let top_line = if !top_right.is_empty() {
            let top_right_str = assemble_no_trail(top_right, theme, sep, false, ctx);
            let top_visible = visible_len(&top_str);
            let right_visible = visible_len(&top_right_str);
            let gap = columns
                .map(|cols| {
                    let used = top_visible + right_visible;
                    if cols as usize > used { cols as usize - used } else { 0 }
                })
                .unwrap_or(1);

            // Use filler char if configured, otherwise plain spaces.
            let fill_str = if let Some(ref filler) = theme.segments.filler {
                let fill_char = &filler.char;
                let repeated = fill_char.repeat(gap);
                if let Some(ref color_name) = filler.color {
                    let cap = capability();
                    if cap != TermCapability::None {
                        let color = SegmentColor {
                            fg: Some(color_name.clone()),
                            bg: None,
                            bold: false,
                        };
                        apply_color_zsh(&repeated, &color, cap)
                    } else {
                        repeated
                    }
                } else {
                    repeated
                }
            } else {
                " ".repeat(gap)
            };
            format!("{top_str}{fill_str}{top_right_str}")
        } else {
            top_str
        };
        // Use ANSI-C quoting ($'\n') to embed a real newline between the two
        // prompt lines. A literal \n inside PROMPT="..." is NOT a newline in
        // zsh — it renders as the two characters '\' and 'n'.
        format!("PROMPT={spacer}\"{top_line}\"$'\\n'\"{left_str}\"\nRPROMPT=\"{rprompt}\"\n")
    };

    if !continuation.is_empty() {
        let prompt2 = assemble(continuation, theme, sep, true, ctx);
        out.push_str(&format!("PROMPT2=\"{prompt2}\"\n"));
    }

    out
}

/// Compute the visible (display) length of a prompt string by stripping
/// zsh zero-width markers `%{...%}` and ANSI escape sequences.
///
/// Uses a byte-level scan so we can look for the exact 2-byte sequence `%}`
/// (the zsh closer) rather than tracking brace depth with chars, which breaks
/// because encountering `%` inside an ANSI escape would wrongly inflate depth.
fn visible_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    let mut len = 0usize;
    while i < b.len() {
        if b[i] == b'%' && i + 1 < b.len() && b[i + 1] == b'{' {
            // Skip zsh zero-width block %{...%}  — search for literal %}.
            i += 2;
            while i + 1 < b.len() {
                if b[i] == b'%' && b[i + 1] == b'}' {
                    i += 2;
                    break;
                }
                i += 1;
            }
        } else if b[i] == 0x1b && i + 1 < b.len() && b[i + 1] == b'[' {
            // Skip ANSI CSI sequence: ESC [ <params> <final 0x40-0x7E>
            i += 2;
            while i < b.len() {
                let byte = b[i];
                i += 1;
                if byte >= 0x40 && byte <= 0x7e { break; }
            }
        } else {
            // Count only the leading byte of each UTF-8 code point.
            if b[i] & 0b1100_0000 != 0b1000_0000 {
                len += 1;
            }
            i += 1;
        }
    }
    len
}

/// Like `assemble` but without the trailing space (used for top_right so padding
/// is controlled externally).
fn assemble_no_trail(
    segs: &[RenderedSegment],
    theme: &Theme,
    sep: &Separators,
    is_left: bool,
    ctx: Option<&crate::segment::RenderContext>,
) -> String {
    let assembled = assemble(segs, theme, sep, is_left, ctx);
    // assemble() always appends a trailing space; strip it for right-side content.
    assembled.strip_suffix(' ').unwrap_or(&assembled).to_string()
}

/// Emit a transient PROMPT (replaces full prompt after command runs).
///
/// Uses `[transient]` theme config when available, falling back to the
/// `prompt_char` segment's symbol for backwards compatibility.
pub fn render_transient_prompt(theme: &Theme) -> String {
    if let Some(ref transient) = theme.transient {
        let cap = capability();
        let text = if cap != TermCapability::None && (transient.fg.is_some() || transient.bg.is_some()) {
            let color = SegmentColor {
                fg: transient.fg.clone(),
                bg: transient.bg.clone(),
                bold: false,
            };
            apply_color_zsh(&transient.template, &color, cap)
        } else {
            transient.template.clone()
        };
        format!("PROMPT=\"{text}\"\nRPROMPT=\"\"\n")
    } else {
        // Legacy fallback: prompt_char symbol.
        let symbol = theme
            .segment
            .get("prompt_char")
            .and_then(|v| v.get("symbol"))
            .and_then(|v| v.as_str())
            .unwrap_or("❯");
        format!("PROMPT=\"{symbol} \"\nRPROMPT=\"\"\n")
    }
}

/// Assemble a prompt string from segments, applying theme colors and separators.
///
/// ANSI escape sequences are wrapped in zsh `%{...%}` so zsh does not count
/// them as visible characters when computing line length. Without this, the
/// cursor position is miscalculated and line editing breaks.
///
/// `is_left` controls which separator (left/right) is used between segments.
/// Resolve the bg color string for a segment from theme config.
fn resolve_seg_bg(seg: &RenderedSegment, theme: &Theme) -> Option<String> {
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
    theme: &Theme,
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

/// Resolve per-segment separator overrides from theme config.
fn resolve_seg_separators(seg: &RenderedSegment, theme: &Theme) -> SegmentSeparators {
    seg.cache_key
        .as_deref()
        .and_then(|name| theme.segment.get(name))
        .and_then(|sc| {
            let leading = sc.get("leading_char").and_then(|v| v.as_str()).map(str::to_string);
            let trailing = sc.get("trailing_char").and_then(|v| v.as_str()).map(str::to_string);
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

fn assemble(segs: &[RenderedSegment], theme: &Theme, sep: &Separators, is_left: bool, ctx: Option<&crate::segment::RenderContext>) -> String {
    if segs.is_empty() {
        return if is_left { "$ ".to_string() } else { String::new() };
    }

    let cap = capability();

    // Resolve color configs and render colored text for each segment.
    // If ctx is provided and the segment has color_when, evaluate conditions.
    let color_cfgs: Vec<Option<SegmentColor>> = segs
        .iter()
        .map(|seg| {
            let seg_config = seg.cache_key
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
                    return Some(crate::evaluator::resolve_conditional_color(base, &color_when, render_ctx));
                }
            }

            base_color
        })
        .collect();

    let parts: Vec<String> = segs
        .iter()
        .zip(color_cfgs.iter())
        .map(|(seg, color_cfg)| {
            if cap != TermCapability::None {
                if let Some(ref color) = color_cfg {
                    apply_color_zsh(&seg.text, color, cap)
                } else {
                    seg.text.clone()
                }
            } else {
                seg.text.clone()
            }
        })
        .collect();

    let glyph = if is_left { &sep.left } else { &sep.right };
    let sep_str = glyph.char.as_deref().unwrap_or(" ");

    // Render left_edge before first segment if configured.
    // In adaptive mode, auto-derive edge fg from first segment's bg when no
    // explicit edge color is set — this creates a smooth "fade-in" cap.
    let edge_glyph = if is_left { &sep.left_edge } else { &sep.right_edge };
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
                    let color = SegmentColor { fg: Some(col), bg: None, bold: false };
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
            let gap_char = seg_seps[i]
                .trailing_char
                .as_deref()
                .unwrap_or(sep_str);
            joined.push_str(&render_gap(gap_char, i, segs, theme, sep, glyph, cap));
        } else {
            // Last segment — emit trailing_char or adaptive tail arrow.
            if let Some(ref trail) = seg_seps[i].trailing_char {
                if cap != TermCapability::None {
                    let fg = resolve_seg_bg(&segs[i], theme);
                    if let Some(col) = fg {
                        let color = SegmentColor { fg: Some(col), bg: None, bold: false };
                        joined.push_str(&apply_color_zsh(trail, &color, cap));
                    } else {
                        joined.push_str(trail);
                    }
                } else {
                    joined.push_str(trail);
                }
            } else if sep.mode == SeparatorMode::Adaptive {
                // Default tail arrow for adaptive mode.
                if let Some(last_bg) = resolve_seg_bg(segs.last().unwrap(), theme) {
                    if cap != TermCapability::None {
                        let color = SegmentColor { fg: Some(last_bg), bg: None, bold: false };
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
    use lynx_theme::{parse_and_validate, terminal::override_capability};

    fn load_default() -> lynx_theme::Theme {
        parse_and_validate(include_str!("../../../themes/default.toml"), "default").unwrap()
    }

    #[test]
    fn render_produces_prompt_and_rprompt() {
        let theme = load_default();
        let left = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let right = vec![RenderedSegment::new("main").with_cache_key("git_branch")];
        let out = render_prompt(&left, &right, &[], &[], &[], &theme, None, None);
        assert!(out.contains("PROMPT="));
        assert!(out.contains("RPROMPT="));
    }

    #[test]
    fn empty_segments_produce_bare_prompt() {
        let theme = load_default();
        let out = render_prompt(&[], &[], &[], &[], &[], &theme, None, None);
        assert!(out.contains("PROMPT="));
    }

    #[test]
    fn two_line_prompt_contains_newline() {
        let theme = load_default();
        let top = vec![RenderedSegment::new("info").with_cache_key("dir")];
        let left = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let out = render_prompt(&left, &[], &top, &[], &[], &theme, None, None);
        assert!(out.contains("$'\\n'"), "expected ANSI-C newline ($'\\n') in two-line prompt");
    }

    #[test]
    fn top_right_padded_to_right_edge() {
        override_capability(TermCapability::None);
        let theme = load_default();
        let top = vec![RenderedSegment::new("info").with_cache_key("dir")];
        let top_right = vec![RenderedSegment::new("[main]").with_cache_key("git_branch")];
        let out = render_prompt(&[], &[], &top, &top_right, &[], &theme, Some(80), None);
        // top_right content must appear in the top line
        assert!(out.contains("[main]"), "expected top_right content in output: {out:?}");
        // padding spaces must appear between top and top_right
        assert!(out.contains("  "), "expected padding spaces in output: {out:?}");
    }

    #[test]
    fn continuation_emits_prompt2() {
        let theme = load_default();
        let cont = vec![RenderedSegment::new("> ").with_cache_key("prompt_char")];
        let out = render_prompt(&[], &[], &[], &[], &cont, &theme, None, None);
        assert!(out.contains("PROMPT2="));
    }

    #[test]
    fn transient_prompt_is_minimal() {
        let theme = load_default();
        let out = render_transient_prompt(&theme);
        assert!(out.contains("PROMPT="));
        assert!(out.contains("RPROMPT=\"\""));
        // should not contain segment content
        assert!(!out.contains("~/code"));
    }

    #[test]
    fn visible_len_strips_zsh_markers_and_ansi() {
        // Plain text
        assert_eq!(visible_len("hello"), 5);
        // zsh %{...%} wrappers are zero-width
        assert_eq!(visible_len("%{\x1b[32m%}hello%{\x1b[0m%}"), 5);
        // ANSI CSI sequence is zero-width
        assert_eq!(visible_len("\x1b[1;32mhello\x1b[0m"), 5);
        // UTF-8 multi-byte counted as 1 char
        assert_eq!(visible_len("~/dev"), 5);
        // Combined: zsh-wrapped ANSI + text after it
        assert_eq!(visible_len("%{\x1b[1m%}┌─[%{\x1b[0m%}foo"), 6); // ┌─[ = 3, foo = 3
        // Trailing space counted
        assert_eq!(visible_len("abc "), 4);
    }

    #[test]
    fn colored_segment_contains_zsh_wrappers() {
        override_capability(TermCapability::Ansi256);

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
        override_capability(TermCapability::None);

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
        override_capability(TermCapability::TrueColor);

        // Build a theme where segments have bg colors.
        let mut theme = load_default();
        // Set dir bg=blue, git_branch bg=green via segment config.
        let dir_color = toml::Value::try_from(toml::toml! {
            color = { fg = "white", bg = "blue" }
        }).unwrap();
        let git_color = toml::Value::try_from(toml::toml! {
            color = { fg = "black", bg = "green" }
        }).unwrap();
        theme.segment.insert("dir".to_string(), dir_color);
        theme.segment.insert("git_branch".to_string(), git_color);

        let mut sep = Separators::default();
        sep.mode = SeparatorMode::Adaptive;
        sep.left.char = Some("\u{e0b0}".to_string()); //

        let segs = vec![
            RenderedSegment::new("~/code").with_cache_key("dir"),
            RenderedSegment::new("main").with_cache_key("git_branch"),
        ];

        let result = assemble(&segs, &theme, &sep, true, None);
        // Separator between dir→git_branch should have fg=blue (dir's bg).
        // blue = (122, 162, 247) → 38;2;122;162;247
        assert!(result.contains("38;2;122;162;247"), "separator fg should be dir's bg (blue): {result:?}");
        // Separator bg should be green (git_branch's bg).
        // green = (158, 206, 106) → 48;2;158;206;106
        assert!(result.contains("48;2;158;206;106"), "separator bg should be git_branch's bg (green): {result:?}");
    }

    #[test]
    fn adaptive_tail_arrow_emitted() {
        override_capability(TermCapability::TrueColor);

        let mut theme = load_default();
        let dir_color = toml::Value::try_from(toml::toml! {
            color = { fg = "white", bg = "blue" }
        }).unwrap();
        theme.segment.insert("dir".to_string(), dir_color);

        let mut sep = Separators::default();
        sep.mode = SeparatorMode::Adaptive;
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
        override_capability(TermCapability::None);

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

        assert_eq!(result_static, result_explicit, "Static mode must be byte-identical to default");
    }

    #[test]
    fn custom_separator_char_is_used() {
        override_capability(TermCapability::None);
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
        override_capability(TermCapability::None);
        let mut theme = load_default();
        // Set leading_char on dir segment.
        let dir_cfg = toml::Value::try_from(toml::toml! {
            leading_char = "\u{e0b6}"
        }).unwrap();
        theme.segment.insert("dir".to_string(), dir_cfg);

        let segs = vec![
            RenderedSegment::new("~/code").with_cache_key("dir"),
        ];
        let result = assemble(&segs, &theme, &theme.separators, true, None);
        assert!(result.contains('\u{e0b6}'), "expected leading char: {result:?}");
        assert!(result.contains("~/code"), "expected segment content: {result:?}");
        // Leading char should appear before the content.
        let lead_pos = result.find('\u{e0b6}').unwrap();
        let content_pos = result.find("~/code").unwrap();
        assert!(lead_pos < content_pos, "leading char should precede content: {result:?}");
    }

    #[test]
    fn per_segment_trailing_char_replaces_gap() {
        override_capability(TermCapability::None);
        let mut theme = load_default();
        // Set trailing_char on dir segment — should replace the gap between dir and git.
        let dir_cfg = toml::Value::try_from(toml::toml! {
            trailing_char = "\u{e0b4}"
        }).unwrap();
        theme.segment.insert("dir".to_string(), dir_cfg);

        let mut sep = Separators::default();
        sep.left.char = Some("|".to_string()); // Global separator.

        let segs = vec![
            RenderedSegment::new("~/code").with_cache_key("dir"),
            RenderedSegment::new("main").with_cache_key("git_branch"),
        ];
        let result = assemble(&segs, &theme, &sep, true, None);
        // The gap between dir and git_branch should use trailing_char, not "|".
        assert!(result.contains('\u{e0b4}'), "expected trailing char in gap: {result:?}");
        // The global "|" should NOT appear between these two segments.
        // (It might not appear at all since there's only one gap.)
        let between = &result[result.find("~/code").unwrap()..result.find("main").unwrap()];
        assert!(!between.contains('|'), "global separator should not appear in gap with trailing_char: {between:?}");
    }

    #[test]
    fn per_segment_trailing_char_on_last_segment() {
        override_capability(TermCapability::None);
        let mut theme = load_default();
        let dir_cfg = toml::Value::try_from(toml::toml! {
            trailing_char = "\u{e0b4}"
        }).unwrap();
        theme.segment.insert("dir".to_string(), dir_cfg);

        let segs = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let result = assemble(&segs, &theme, &theme.separators, true, None);
        assert!(result.contains('\u{e0b4}'), "expected trailing char after last segment: {result:?}");
    }

    #[test]
    fn diamond_segment_mixed_with_powerline() {
        override_capability(TermCapability::None);
        let mut theme = load_default();
        // Shell segment gets diamond caps.
        let shell_cfg = toml::Value::try_from(toml::toml! {
            leading_char = "\u{e0b6}"
            trailing_char = "\u{e0b4}"
        }).unwrap();
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
        assert!(result.contains('\u{e0b6}'), "expected leading diamond: {result:?}");
        assert!(result.contains('\u{e0b4}'), "expected trailing diamond: {result:?}");
        // Dir should NOT have diamond caps.
        assert!(result.contains("~/code"), "expected dir content: {result:?}");
    }

    #[test]
    fn filler_replaces_spaces_on_top_line() {
        override_capability(TermCapability::None);
        let mut theme = load_default();
        theme.segments.filler = Some(lynx_theme::schema::FillerConfig {
            char: "─".to_string(),
            color: None,
        });
        let top = vec![RenderedSegment::new("left").with_cache_key("dir")];
        let top_right = vec![RenderedSegment::new("right").with_cache_key("git_branch")];
        let out = render_prompt(&[], &[], &top, &top_right, &[], &theme, Some(40), None);
        assert!(out.contains("─"), "expected filler char in top line: {out:?}");
        assert!(!out.contains("     "), "should not have long space runs: {out:?}");
    }

    #[test]
    fn transient_prompt_uses_theme_config() {
        override_capability(TermCapability::None);
        let mut theme = load_default();
        theme.transient = Some(lynx_theme::schema::TransientConfig {
            template: "→ ".to_string(),
            fg: None,
            bg: None,
        });
        let out = render_transient_prompt(&theme);
        assert!(out.contains("→"), "expected custom transient template: {out:?}");
        assert!(!out.contains("❯"), "should not contain default symbol: {out:?}");
    }

    #[test]
    fn transient_prompt_falls_back_to_prompt_char() {
        let theme = load_default();
        // No [transient] config — should use prompt_char symbol.
        let out = render_transient_prompt(&theme);
        assert!(out.contains("PROMPT="), "expected PROMPT assignment: {out:?}");
    }

    #[test]
    fn no_per_segment_overrides_matches_original_behavior() {
        // Verify that when no segments have leading_char/trailing_char,
        // the output is identical to the old behavior.
        override_capability(TermCapability::None);
        let theme = load_default();
        let segs = vec![
            RenderedSegment::new("a").with_cache_key("dir"),
            RenderedSegment::new("b").with_cache_key("git_branch"),
        ];
        let result = assemble(&segs, &theme, &theme.separators, true, None);
        // With no separator config, segments are space-separated.
        assert_eq!(result, "a b ", "default behavior should be space-separated: {result:?}");
    }
}
