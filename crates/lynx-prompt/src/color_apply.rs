use lynx_theme::{
    color::Color,
    schema::SegmentColor,
    terminal::{capability, TermCapability},
};

/// Apply color + bold + bg to text, wrapping ANSI sequences in zsh `%{...%}`.
///
/// The `%{` / `%}` markers tell zsh that the enclosed bytes are zero-width,
/// so line length computation stays correct. They are zsh-specific and must
/// only appear in strings that zsh will interpret as a prompt (PROMPT/RPROMPT).
pub(crate) fn apply_color_zsh(text: &str, color: &SegmentColor, cap: TermCapability) -> String {
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

    if let Some(bg) = &color.bg {
        let c = if bg.starts_with('#') {
            Color::Hex(bg.clone())
        } else {
            Color::Named(bg.clone())
        };
        let esc = c.render_bg(cap);
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
pub(crate) fn zsh_wrap(esc: &str) -> String {
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

    if let Some(bg) = &color.bg {
        let c = if bg.starts_with('#') {
            Color::Hex(bg.clone())
        } else {
            Color::Named(bg.clone())
        };
        out.push_str(&c.render_bg(cap));
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
    use lynx_theme::terminal::override_capability;

    #[test]
    fn segment_with_fg_and_bg_emits_both_codes() {
        override_capability(TermCapability::TrueColor);

        let color = SegmentColor {
            fg: Some("white".to_string()),
            bg: Some("blue".to_string()),
            bold: false,
        };
        let result = apply_color_zsh("test", &color, TermCapability::TrueColor);
        assert!(result.contains("38;"), "expected fg (38;) code in: {result:?}");
        assert!(result.contains("48;"), "expected bg (48;) code in: {result:?}");
        assert!(result.contains("\x1b[0m"), "expected reset in: {result:?}");
    }

    #[test]
    fn colorize_emits_bg_when_set() {
        override_capability(TermCapability::TrueColor);

        let color = SegmentColor {
            fg: Some("white".to_string()),
            bg: Some("blue".to_string()),
            bold: false,
        };
        let result = colorize("test", &color);
        assert!(result.contains("38;"), "expected fg code in colorize: {result:?}");
        assert!(result.contains("48;"), "expected bg code in colorize: {result:?}");
    }
}
