use lynx_theme::{
    color::Color,
    schema::{SegmentColor, Theme},
    terminal::{capability, TermCapability},
};

use crate::segment::RenderedSegment;

/// Assemble PROMPT and RPROMPT shell assignments from rendered segments.
pub fn render_prompt(left: &[RenderedSegment], right: &[RenderedSegment], theme: &Theme) -> String {
    let prompt = assemble(left, theme);
    let rprompt = assemble(right, theme);

    // Output as shell assignments for eval by precmd hook.
    format!("PROMPT=\"{prompt}\"\nRPROMPT=\"{rprompt}\"\n")
}

/// Assemble a prompt string from segments, applying theme colors.
///
/// ANSI escape sequences are wrapped in zsh `%{...%}` so zsh does not count
/// them as visible characters when computing line length. Without this, the
/// cursor position is miscalculated and line editing breaks.
fn assemble(segs: &[RenderedSegment], theme: &Theme) -> String {
    if segs.is_empty() {
        return "$ ".to_string();
    }

    let cap = capability();
    let mut parts: Vec<String> = Vec::with_capacity(segs.len());

    for seg in segs {
        // Look up color config by the segment's cache_key (which is the segment name).
        let color_cfg = seg
            .cache_key
            .as_deref()
            .and_then(|name| theme.segment.get(name))
            .and_then(|sc| sc.color.as_ref());

        let text = if cap != TermCapability::None {
            if let Some(color) = color_cfg {
                apply_color_zsh(&seg.text, color, cap)
            } else {
                seg.text.clone()
            }
        } else {
            seg.text.clone()
        };

        parts.push(text);
    }

    format!("{} $ ", parts.join(" "))
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
        let out = render_prompt(&left, &right, &theme);
        assert!(out.contains("PROMPT="));
        assert!(out.contains("RPROMPT="));
    }

    #[test]
    fn empty_segments_produce_bare_prompt() {
        let theme = load("default").unwrap();
        let out = render_prompt(&[], &[], &theme);
        assert!(out.contains("PROMPT="));
    }

    #[test]
    fn colored_segment_contains_zsh_wrappers() {
        override_capability(TermCapability::Ansi256);

        let theme = load("default").unwrap();
        // dir segment has color = { fg = "blue", bold = true } in default theme
        let segs = vec![RenderedSegment::new("~/code").with_cache_key("dir")];
        let result = assemble(&segs, &theme);

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
        let result = assemble(&segs, &theme);

        assert!(
            !result.contains("\x1b["),
            "expected no ANSI escapes in: {result:?}"
        );
    }
}
