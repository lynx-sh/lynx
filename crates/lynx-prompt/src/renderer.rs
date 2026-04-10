use lynx_theme::{
    color::Color,
    schema::{SegmentColor, Theme},
    terminal::capability,
};

use crate::segment::RenderedSegment;

/// Assemble PROMPT and RPROMPT shell assignments from rendered segments.
pub fn render_prompt(
    left: &[RenderedSegment],
    right: &[RenderedSegment],
    theme: &Theme,
) -> String {
    let prompt = assemble(left, theme);
    let rprompt = assemble(right, theme);

    // Output as shell assignments for eval by precmd hook.
    format!("PROMPT={prompt:?}\nRPROMPT={rprompt:?}\n")
}

fn assemble(segs: &[RenderedSegment], _theme: &Theme) -> String {
    if segs.is_empty() {
        return "$ ".to_string();
    }
    let parts: Vec<&str> = segs.iter().map(|s| s.text.as_str()).collect();
    format!("{} $ ", parts.join(" "))
}

/// Apply a SegmentColor from theme config to a text string.
pub fn colorize(text: &str, color: &SegmentColor) -> String {
    let cap = capability();
    let mut out = String::new();

    if let Some(fg) = &color.fg {
        // Try named color first, then hex.
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
    use lynx_theme::loader::load;

    #[test]
    fn render_produces_prompt_and_rprompt() {
        let theme = load("default").unwrap();
        let left = vec![RenderedSegment::new("~/code")];
        let right = vec![RenderedSegment::new("main")];
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
}
