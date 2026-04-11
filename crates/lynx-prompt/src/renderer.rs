use lynx_theme::{
    color::Color,
    schema::{SegmentColor, Separators, Theme},
    terminal::{capability, TermCapability},
};

use crate::segment::RenderedSegment;

/// Assemble PROMPT and RPROMPT shell assignments from rendered segments.
///
/// When `top` is non-empty the output is a two-line prompt:
///   PROMPT="<top line>\n<left line> "
///
/// When `continuation` is non-empty, PROMPT2 is also emitted.
pub fn render_prompt(
    left: &[RenderedSegment],
    right: &[RenderedSegment],
    top: &[RenderedSegment],
    continuation: &[RenderedSegment],
    theme: &Theme,
) -> String {
    let sep = &theme.separators;

    // Build the input-line part of PROMPT (left segments).
    let left_str = assemble(left, theme, sep, true);
    let rprompt = assemble(right, theme, sep, false);

    let prompt = if top.is_empty() {
        left_str
    } else {
        let top_str = assemble(top, theme, sep, true);
        // Embed a literal newline inside the zsh PROMPT string.
        // The $'\n' syntax works inside double-quoted zsh assignments.
        format!("{top_str}\\n{left_str}")
    };

    let mut out = format!("PROMPT=\"{prompt}\"\nRPROMPT=\"{rprompt}\"\n");

    if !continuation.is_empty() {
        let prompt2 = assemble(continuation, theme, sep, true);
        out.push_str(&format!("PROMPT2=\"{prompt2}\"\n"));
    }

    out
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
        let out = render_prompt(&left, &right, &[], &[], &theme);
        assert!(out.contains("PROMPT="));
        assert!(out.contains("RPROMPT="));
    }

    #[test]
    fn empty_segments_produce_bare_prompt() {
        let theme = load("default").unwrap();
        let out = render_prompt(&[], &[], &[], &[], &theme);
        assert!(out.contains("PROMPT="));
    }

    #[test]
    fn two_line_prompt_contains_newline() {
        let theme = load("default").unwrap();
        let top = vec![RenderedSegment::new("info").with_cache_key("dir")];
        let left = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let out = render_prompt(&left, &[], &top, &[], &theme);
        assert!(out.contains("\\n"), "expected embedded newline in two-line prompt");
    }

    #[test]
    fn continuation_emits_prompt2() {
        let theme = load("default").unwrap();
        let cont = vec![RenderedSegment::new("> ").with_cache_key("prompt_char")];
        let out = render_prompt(&[], &[], &[], &cont, &theme);
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
