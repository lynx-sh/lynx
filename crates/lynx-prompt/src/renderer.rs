use lynx_theme::{
    schema::{SegmentColor, SeparatorMode, Separators, Theme},
    terminal::{capability, TermCapability},
};

use crate::assemble::{assemble, assemble_no_trail};
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
#[allow(clippy::too_many_arguments)]
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
        let plain_sep = Separators {
            mode: SeparatorMode::Static,
            ..Separators::default()
        };
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
                    (cols as usize).saturating_sub(used)
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

    // PROMPT2 — continuation prompt for multi-line input.
    // Template mode takes priority; falls back to assembled segments.
    let cont_cfg = &theme.segments.continuation;
    if let Some(ref tmpl) = cont_cfg.template {
        let cap = capability();
        let text =
            if cap != TermCapability::None && (cont_cfg.fg.is_some() || cont_cfg.bg.is_some()) {
                let color = SegmentColor {
                    fg: cont_cfg.fg.clone(),
                    bg: cont_cfg.bg.clone(),
                    bold: false,
                };
                apply_color_zsh(tmpl, &color, cap)
            } else {
                tmpl.clone()
            };
        out.push_str(&format!("PROMPT2=\"{text}\"\n"));
    } else if !continuation.is_empty() {
        let prompt2 = assemble(continuation, theme, sep, true, ctx);
        out.push_str(&format!("PROMPT2=\"{prompt2}\"\n"));
    }

    // PROMPT4 — debug/xtrace prompt for `set -x` output.
    if let Some(ref dbg) = theme.debug_prompt {
        let cap = capability();
        let text = if cap != TermCapability::None && (dbg.fg.is_some() || dbg.bg.is_some()) {
            let color = SegmentColor {
                fg: dbg.fg.clone(),
                bg: dbg.bg.clone(),
                bold: false,
            };
            apply_color_zsh(&dbg.template, &color, cap)
        } else {
            dbg.template.clone()
        };
        out.push_str(&format!("PROMPT4=\"{text}\"\n"));
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
                if (0x40..=0x7e).contains(&byte) {
                    break;
                }
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

/// Emit a transient PROMPT (replaces full prompt after command runs).
///
/// Uses `[transient]` theme config when available, falling back to the
/// `prompt_char` segment's symbol for backwards compatibility.
pub fn render_transient_prompt(theme: &Theme) -> String {
    if let Some(ref transient) = theme.transient {
        let cap = capability();
        let text =
            if cap != TermCapability::None && (transient.fg.is_some() || transient.bg.is_some()) {
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
        assert!(
            out.contains("$'\\n'"),
            "expected ANSI-C newline ($'\\n') in two-line prompt"
        );
    }

    #[test]
    fn top_right_padded_to_right_edge() {
        override_capability(TermCapability::None);
        let theme = load_default();
        let top = vec![RenderedSegment::new("info").with_cache_key("dir")];
        let top_right = vec![RenderedSegment::new("[main]").with_cache_key("git_branch")];
        let out = render_prompt(&[], &[], &top, &top_right, &[], &theme, Some(80), None);
        // top_right content must appear in the top line
        assert!(
            out.contains("[main]"),
            "expected top_right content in output: {out:?}"
        );
        // padding spaces must appear between top and top_right
        assert!(
            out.contains("  "),
            "expected padding spaces in output: {out:?}"
        );
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
        assert!(
            out.contains("─"),
            "expected filler char in top line: {out:?}"
        );
        assert!(
            !out.contains("     "),
            "should not have long space runs: {out:?}"
        );
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
        assert!(
            out.contains("→"),
            "expected custom transient template: {out:?}"
        );
        assert!(
            !out.contains("❯"),
            "should not contain default symbol: {out:?}"
        );
    }

    #[test]
    fn transient_prompt_falls_back_to_prompt_char() {
        let theme = load_default();
        // No [transient] config — should use prompt_char symbol.
        let out = render_transient_prompt(&theme);
        assert!(
            out.contains("PROMPT="),
            "expected PROMPT assignment: {out:?}"
        );
    }

    #[test]
    fn continuation_template_emits_prompt2_without_segments() {
        override_capability(TermCapability::None);
        let mut theme = load_default();
        theme.segments.continuation = lynx_theme::schema::ContinuationConfig {
            order: vec![],
            template: Some("… ".to_string()),
            fg: None,
            bg: None,
        };
        let out = render_prompt(&[], &[], &[], &[], &[], &theme, None, None);
        assert!(
            out.contains("PROMPT2=\"… \""),
            "expected template in PROMPT2: {out:?}"
        );
    }

    #[test]
    fn continuation_template_takes_priority_over_segments() {
        override_capability(TermCapability::None);
        let mut theme = load_default();
        theme.segments.continuation = lynx_theme::schema::ContinuationConfig {
            order: vec![],
            template: Some("> ".to_string()),
            fg: None,
            bg: None,
        };
        // segments slice is non-empty but template should win
        let cont = vec![RenderedSegment::new("other").with_cache_key("x")];
        let out = render_prompt(&[], &[], &[], &[], &cont, &theme, None, None);
        assert!(
            out.contains("PROMPT2=\"> \""),
            "template should override segments: {out:?}"
        );
        assert!(
            !out.contains("other"),
            "segment content should not appear when template is set: {out:?}"
        );
    }

    #[test]
    fn continuation_segments_still_work_without_template() {
        let theme = load_default();
        let cont = vec![RenderedSegment::new("> ").with_cache_key("prompt_char")];
        let out = render_prompt(&[], &[], &[], &[], &cont, &theme, None, None);
        assert!(out.contains("PROMPT2="), "expected PROMPT2: {out:?}");
    }

    #[test]
    fn debug_prompt_emits_prompt4() {
        override_capability(TermCapability::None);
        let mut theme = load_default();
        theme.debug_prompt = Some(lynx_theme::schema::DebugPromptConfig {
            template: "+ ".to_string(),
            fg: None,
            bg: None,
        });
        let out = render_prompt(&[], &[], &[], &[], &[], &theme, None, None);
        assert!(
            out.contains("PROMPT4=\"+ \""),
            "expected PROMPT4 assignment: {out:?}"
        );
    }

    #[test]
    fn debug_prompt_absent_no_prompt4() {
        let theme = load_default();
        // No debug_prompt config — PROMPT4 must not appear.
        let out = render_prompt(&[], &[], &[], &[], &[], &theme, None, None);
        assert!(
            !out.contains("PROMPT4"),
            "PROMPT4 should not appear without debug_prompt config: {out:?}"
        );
    }
}
