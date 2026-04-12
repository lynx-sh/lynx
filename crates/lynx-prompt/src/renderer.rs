use lynx_theme::{
    schema::{SegmentColor, SeparatorMode, Separators, Theme},
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
) -> String {
    let sep = &theme.separators;

    // Build the input-line part of PROMPT (left segments).
    // When a top line exists, the bottom (input) line should render plain —
    // no powerline edge glyphs, no adaptive separators.
    let left_str = if !top.is_empty() {
        let mut plain_sep = Separators::default();
        plain_sep.mode = SeparatorMode::Static;
        assemble(left, theme, &plain_sep, true)
    } else {
        assemble(left, theme, sep, true)
    };
    let rprompt = assemble(right, theme, sep, false);

    // Optional blank line before the prompt for visual breathing room.
    let spacer = if theme.segments.spacing { "$'\\n'" } else { "" };

    let mut out = if top.is_empty() {
        format!("PROMPT={spacer}\"{left_str}\"\nRPROMPT=\"{rprompt}\"\n")
    } else {
        let top_str = assemble(top, theme, sep, true);
        let top_line = if !top_right.is_empty() {
            let top_right_str = assemble_no_trail(top_right, theme, sep, false);
            let top_visible = visible_len(&top_str);
            let right_visible = visible_len(&top_right_str);
            let padding = columns
                .map(|cols| {
                    let used = top_visible + right_visible;
                    if cols as usize > used { cols as usize - used } else { 0 }
                })
                .unwrap_or(1);
            format!("{top_str}{}{top_right_str}", " ".repeat(padding))
        } else {
            top_str
        };
        // Use ANSI-C quoting ($'\n') to embed a real newline between the two
        // prompt lines. A literal \n inside PROMPT="..." is NOT a newline in
        // zsh — it renders as the two characters '\' and 'n'.
        format!("PROMPT={spacer}\"{top_line}\"$'\\n'\"{left_str}\"\nRPROMPT=\"{rprompt}\"\n")
    };

    if !continuation.is_empty() {
        let prompt2 = assemble(continuation, theme, sep, true);
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
) -> String {
    let assembled = assemble(segs, theme, sep, is_left);
    // assemble() always appends a trailing space; strip it for right-side content.
    assembled.strip_suffix(' ').unwrap_or(&assembled).to_string()
}

/// Emit a minimal transient PROMPT (replaces full prompt after command runs).
/// Just outputs `PROMPT="<symbol> "` — the symbol is taken from the
/// `prompt_char` segment config or defaults to `❯`.
pub fn render_transient_prompt(theme: &Theme) -> String {
    let symbol = theme
        .segment
        .get("prompt_char")
        .and_then(|v| v.get("symbol"))
        .and_then(|v| v.as_str())
        .unwrap_or("❯");
    format!("PROMPT=\"{symbol} \"\nRPROMPT=\"\"\n")
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

fn assemble(segs: &[RenderedSegment], theme: &Theme, sep: &Separators, is_left: bool) -> String {
    if segs.is_empty() {
        return if is_left { "$ ".to_string() } else { String::new() };
    }

    let cap = capability();

    // Resolve color configs and render colored text for each segment.
    let color_cfgs: Vec<Option<SegmentColor>> = segs
        .iter()
        .map(|seg| {
            seg.cache_key
                .as_deref()
                .and_then(|name| theme.segment.get(name))
                .and_then(|sc| sc.get("color"))
                .and_then(|c| c.clone().try_into().ok())
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
                bg: None,
            };
            apply_color_zsh(edge_str, &color, cap)
        } else {
            edge_str.to_string()
        }
    } else {
        edge_str.to_string()
    };

    let joined = match sep.mode {
        SeparatorMode::Static => {
            // Original behavior: one global separator for all gaps.
            let sep_rendered = if cap != TermCapability::None {
                if let Some(ref col) = glyph.color {
                    let color = SegmentColor {
                        fg: Some(col.clone()),
                        bold: false,
                        bg: None,
                    };
                    apply_color_zsh(sep_str, &color, cap)
                } else {
                    sep_str.to_string()
                }
            } else {
                sep_str.to_string()
            };
            parts.join(&sep_rendered)
        }
        SeparatorMode::Adaptive => {
            // Per-gap separator: fg = prev segment bg, bg = next segment bg.
            let mut result = String::new();
            for (i, part) in parts.iter().enumerate() {
                result.push_str(part);
                if i + 1 < parts.len() {
                    let prev_bg = resolve_seg_bg(&segs[i], theme);
                    let next_bg = resolve_seg_bg(&segs[i + 1], theme);
                    if cap != TermCapability::None && prev_bg.is_some() {
                        let color = SegmentColor {
                            fg: prev_bg,
                            bg: next_bg,
                            bold: false,
                        };
                        result.push_str(&apply_color_zsh(sep_str, &color, cap));
                    } else if cap != TermCapability::None {
                        // No prev bg — fall back to glyph's static color if set.
                        if let Some(ref col) = glyph.color {
                            let color = SegmentColor {
                                fg: Some(col.clone()),
                                bold: false,
                                bg: None,
                            };
                            result.push_str(&apply_color_zsh(sep_str, &color, cap));
                        } else {
                            result.push_str(sep_str);
                        }
                    } else {
                        result.push_str(sep_str);
                    }
                }
            }
            // Tail arrow: if last segment has bg, emit separator with fg=last_bg, no bg.
            if let Some(last_bg) = resolve_seg_bg(segs.last().unwrap(), theme) {
                if cap != TermCapability::None {
                    let color = SegmentColor {
                        fg: Some(last_bg),
                        bg: None,
                        bold: false,
                    };
                    result.push_str(&apply_color_zsh(sep_str, &color, cap));
                }
            }
            result
        }
    };

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
        let out = render_prompt(&left, &right, &[], &[], &[], &theme, None);
        assert!(out.contains("PROMPT="));
        assert!(out.contains("RPROMPT="));
    }

    #[test]
    fn empty_segments_produce_bare_prompt() {
        let theme = load_default();
        let out = render_prompt(&[], &[], &[], &[], &[], &theme, None);
        assert!(out.contains("PROMPT="));
    }

    #[test]
    fn two_line_prompt_contains_newline() {
        let theme = load_default();
        let top = vec![RenderedSegment::new("info").with_cache_key("dir")];
        let left = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let out = render_prompt(&left, &[], &top, &[], &[], &theme, None);
        assert!(out.contains("$'\\n'"), "expected ANSI-C newline ($'\\n') in two-line prompt");
    }

    #[test]
    fn top_right_padded_to_right_edge() {
        override_capability(TermCapability::None);
        let theme = load_default();
        let top = vec![RenderedSegment::new("info").with_cache_key("dir")];
        let top_right = vec![RenderedSegment::new("[main]").with_cache_key("git_branch")];
        let out = render_prompt(&[], &[], &top, &top_right, &[], &theme, Some(80));
        // top_right content must appear in the top line
        assert!(out.contains("[main]"), "expected top_right content in output: {out:?}");
        // padding spaces must appear between top and top_right
        assert!(out.contains("  "), "expected padding spaces in output: {out:?}");
    }

    #[test]
    fn continuation_emits_prompt2() {
        let theme = load_default();
        let cont = vec![RenderedSegment::new("> ").with_cache_key("prompt_char")];
        let out = render_prompt(&[], &[], &[], &[], &cont, &theme, None);
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
        let result = assemble(&segs, &theme, &theme.separators, true);

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
        let result = assemble(&segs, &theme, &theme.separators, true);

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

        let result = assemble(&segs, &theme, &sep, true);
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
        let result = assemble(&segs, &theme, &sep, true);
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
        let result_static = assemble(&segs, &theme, &theme.separators, true);

        // Explicitly set Static.
        let mut sep = theme.separators.clone();
        sep.mode = SeparatorMode::Static;
        let result_explicit = assemble(&segs, &theme, &sep, true);

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
        let result = assemble(&segs, &theme, &sep, true);
        assert!(result.contains('|'), "expected | separator in: {result:?}");
    }
}
