use lynx_theme::{
    color::Color,
    schema::{SegmentColor, Separators, Theme},
    terminal::{capability, TermCapability},
};

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
    let left_str = assemble(left, theme, sep, true);
    let rprompt = assemble(right, theme, sep, false);

    let mut out = if top.is_empty() {
        format!("PROMPT=\"{left_str}\"\nRPROMPT=\"{rprompt}\"\n")
    } else {
        let top_str = assemble(top, theme, sep, true);
        let top_line = if !top_right.is_empty() {
            let top_right_str = assemble_no_trail(top_right, theme, sep, true);
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
        format!("PROMPT=\"{top_line}\"$'\\n'\"{left_str}\"\nRPROMPT=\"{rprompt}\"\n")
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
fn assemble(segs: &[RenderedSegment], theme: &Theme, sep: &Separators, is_left: bool) -> String {
    if segs.is_empty() {
        return if is_left { "$ ".to_string() } else { String::new() };
    }

    let cap = capability();
    let mut parts: Vec<String> = Vec::with_capacity(segs.len());

    for seg in segs {
        // Look up color config by the segment's cache_key (which is the segment name).
        let color_cfg: Option<lynx_theme::schema::SegmentColor> = seg
            .cache_key
            .as_deref()
            .and_then(|name| theme.segment.get(name))
            .and_then(|sc| sc.get("color"))
            .and_then(|c| c.clone().try_into().ok());

        let text = if cap != TermCapability::None {
            if let Some(ref color) = color_cfg {
                apply_color_zsh(&seg.text, color, cap)
            } else {
                seg.text.clone()
            }
        } else {
            seg.text.clone()
        };

        parts.push(text);
    }

    // Determine the separator string between segments.
    let glyph = if is_left { &sep.left } else { &sep.right };
    let sep_str = glyph.char.as_deref().unwrap_or(" ");

    // Apply separator color if configured.
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

    // Render left_edge before first segment if configured.
    let edge_glyph = if is_left { &sep.left_edge } else { &sep.right_edge };
    let edge_str = edge_glyph.char.as_deref().unwrap_or("");
    let edge_rendered = if !edge_str.is_empty() && cap != TermCapability::None {
        if let Some(ref col) = edge_glyph.color {
            let color = SegmentColor {
                fg: Some(col.clone()),
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

    let joined = parts.join(&sep_rendered);
    if edge_rendered.is_empty() {
        format!("{joined} ")
    } else {
        format!("{edge_rendered}{joined} ")
    }
}

/// Apply color + bold to text, wrapping ANSI sequences in zsh `%{...%}`.
///
/// The `%{` / `%}` markers tell zsh that the enclosed bytes are zero-width,
/// so line length computation stays correct. They are zsh-specific and must
/// only appear in strings that zsh will interpret as a prompt (PROMPT/RPROMPT).
fn apply_color_zsh(text: &str, color: &SegmentColor, cap: TermCapability) -> String {
    let mut prefix = String::new();

    if let Some(fg) = &color.fg {
        let c = if fg.starts_with('#') {
            Color::Hex(fg.clone())
        } else {
            Color::Named(fg.clone())
        };
        let esc = c.render_fg(cap);
        if !esc.is_empty() {
            prefix.push_str(&zsh_wrap(&esc));
        }
    }

    if color.bold {
        prefix.push_str(&zsh_wrap("\x1b[1m"));
    }

    if prefix.is_empty() {
        return text.to_string();
    }

    let reset = zsh_wrap(Color::reset());
    format!("{prefix}{text}{reset}")
}

/// Wrap an ANSI escape string in zsh zero-width markers `%{...%}`.
#[inline]
fn zsh_wrap(esc: &str) -> String {
    format!("%{{{esc}%}}")
}

/// Apply a SegmentColor from theme config to a text string (no zsh wrapping).
/// Used outside of prompt contexts (e.g. lx doctor output).
pub fn colorize(text: &str, color: &SegmentColor) -> String {
    let cap = capability();
    let mut out = String::new();

    if let Some(fg) = &color.fg {
        let c = if fg.starts_with('#') {
            Color::Hex(fg.clone())
        } else {
            Color::Named(fg.clone())
        };
        out.push_str(&c.render_fg(cap));
    }

    if color.bold {
        out.push_str("\x1b[1m");
    }

    out.push_str(text);
    out.push_str(Color::reset());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_theme::{loader::load, terminal::override_capability};

    #[test]
    fn render_produces_prompt_and_rprompt() {
        let theme = load("default").unwrap();
        let left = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let right = vec![RenderedSegment::new("main").with_cache_key("git_branch")];
        let out = render_prompt(&left, &right, &[], &[], &[], &theme, None);
        assert!(out.contains("PROMPT="));
        assert!(out.contains("RPROMPT="));
    }

    #[test]
    fn empty_segments_produce_bare_prompt() {
        let theme = load("default").unwrap();
        let out = render_prompt(&[], &[], &[], &[], &[], &theme, None);
        assert!(out.contains("PROMPT="));
    }

    #[test]
    fn two_line_prompt_contains_newline() {
        let theme = load("default").unwrap();
        let top = vec![RenderedSegment::new("info").with_cache_key("dir")];
        let left = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let out = render_prompt(&left, &[], &top, &[], &[], &theme, None);
        assert!(out.contains("$'\\n'"), "expected ANSI-C newline ($'\\n') in two-line prompt");
    }

    #[test]
    fn top_right_padded_to_right_edge() {
        override_capability(TermCapability::None);
        let theme = load("default").unwrap();
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
        let theme = load("default").unwrap();
        let cont = vec![RenderedSegment::new("> ").with_cache_key("prompt_char")];
        let out = render_prompt(&[], &[], &[], &[], &cont, &theme, None);
        assert!(out.contains("PROMPT2="));
    }

    #[test]
    fn transient_prompt_is_minimal() {
        let theme = load("default").unwrap();
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

        let theme = load("default").unwrap();
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

        let theme = load("default").unwrap();
        let segs = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let result = assemble(&segs, &theme, &theme.separators, true);

        assert!(
            !result.contains("\x1b["),
            "expected no ANSI escapes in: {result:?}"
        );
    }

    #[test]
    fn custom_separator_char_is_used() {
        override_capability(TermCapability::None);
        let theme = load("default").unwrap();
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
