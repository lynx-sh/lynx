use std::collections::HashMap;

use crate::schema::{Block, Intro};

/// Render all blocks in an intro to a plain ANSI string ready to print to stdout.
///
/// Unknown `{{TOKEN}}` placeholders resolve to empty string.
/// Rendering errors on individual blocks are logged and skipped — never panics.
pub fn render_intro(intro: &Intro, tokens: &HashMap<String, String>) -> String {
    let mut out = String::new();
    for block in &intro.blocks {
        match render_block(block, tokens) {
            Ok(text) => {
                out.push_str(&text);
                out.push('\n');
            }
            Err(e) => {
                tracing::warn!("intro: skipping block due to render error: {e}");
            }
        }
    }
    out
}

fn render_block(block: &Block, tokens: &HashMap<String, String>) -> anyhow::Result<String> {
    match block {
        Block::Text {
            content,
            color,
            bold,
        } => {
            let text = substitute(content, tokens);
            Ok(apply_style(&text, color.as_deref(), *bold))
        }

        Block::KeyVal {
            items,
            color_key,
            color_val,
        } => {
            if items.is_empty() {
                return Ok(String::new());
            }
            // Align: find longest key for column padding.
            let max_key_len = items.iter().map(|[k, _]| k.len()).max().unwrap_or(0);
            let mut lines = Vec::with_capacity(items.len());
            for [key, val] in items {
                let val_rendered = substitute(val, tokens);
                let key_col = format!("{key:<max_key_len$}");
                let key_part = apply_style(&key_col, color_key.as_deref(), false);
                let val_part = apply_style(&val_rendered, color_val.as_deref(), false);
                lines.push(format!("  {key_part}  {val_part}"));
            }
            Ok(lines.join("\n"))
        }

        Block::Separator { char, width, color } => {
            let line = char.repeat(*width);
            Ok(apply_style(&line, color.as_deref(), false))
        }

        Block::AsciiLogo { font, text, color } => {
            let ascii = crate::figlet::render_ascii(font, text)?;
            Ok(apply_style(&ascii, color.as_deref(), false))
        }
    }
}

/// Substitute `{{TOKEN}}` in `template` using `tokens`.
/// Unknown tokens resolve to empty string (lenient — does not error).
fn substitute(template: &str, tokens: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        rest = &rest[open + 2..];
        if let Some(close) = rest.find("}}") {
            let key = &rest[..close];
            if let Some(val) = tokens.get(key) {
                out.push_str(val);
            }
            // Unknown token → empty (swallowed silently)
            rest = &rest[close + 2..];
        } else {
            // Unclosed {{ — emit as-is and stop substituting
            out.push_str("{{");
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Apply ANSI foreground color and optional bold to `text`.
/// Returns `text` unchanged if color is None/unknown.
fn apply_style(text: &str, color: Option<&str>, bold: bool) -> String {
    let color_esc = color.and_then(ansi_fg);
    let bold_esc = if bold { "\x1b[1m" } else { "" };
    let reset = if color_esc.is_some() || bold {
        "\x1b[0m"
    } else {
        ""
    };

    match color_esc {
        Some(fg) => format!("{bold_esc}{fg}{text}{reset}"),
        None if bold => format!("{bold_esc}{text}{reset}"),
        None => text.to_string(),
    }
}

/// Resolve a color string to an ANSI foreground escape sequence.
/// Supports hex (#RRGGBB) and a set of named colors. Returns None for unknown.
fn ansi_fg(color: &str) -> Option<String> {
    if let Some(hex) = color.strip_prefix('#') {
        let (r, g, b) = parse_hex(hex)?;
        return Some(format!("\x1b[38;2;{r};{g};{b}m"));
    }
    // Named colors — curated palette matching lynx-theme's names
    let (r, g, b) = match color.to_lowercase().as_str() {
        "black" => (0, 0, 0),
        "red" => (247, 118, 142),
        "green" => (158, 206, 106),
        "yellow" => (224, 175, 104),
        "blue" => (122, 162, 247),
        "magenta" => (187, 154, 247),
        "cyan" => (125, 207, 255),
        "white" => (192, 202, 245),
        "bright-red" => (255, 85, 85),
        "bright-green" => (80, 250, 123),
        "bright-yellow" => (241, 250, 140),
        "light-blue" => (130, 170, 255),
        "orange" => (255, 158, 100),
        "muted" | "gray" | "grey" => (128, 128, 128),
        "accent" => (122, 162, 247), // default accent = blue
        _ => return None,
    };
    Some(format!("\x1b[38;2;{r};{g};{b}m"))
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Block, DisplayConfig, Intro, IntroMeta};

    fn fixture_intro() -> Intro {
        Intro {
            meta: IntroMeta {
                name: "test".into(),
                description: "".into(),
                author: "".into(),
            },
            display: DisplayConfig::default(),
            blocks: vec![
                Block::Text {
                    content: "Hello, {{username}}!".into(),
                    color: Some("green".into()),
                    bold: true,
                },
                Block::Separator {
                    char: "─".into(),
                    width: 20,
                    color: None,
                },
                Block::KeyVal {
                    items: vec![
                        ["OS".into(), "{{os}}".into()],
                        ["Shell".into(), "{{shell}}".into()],
                    ],
                    color_key: Some("muted".into()),
                    color_val: None,
                },
            ],
        }
    }

    fn tokens() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("username".into(), "proxikal".into());
        m.insert("os".into(), "macOS 15".into());
        m.insert("shell".into(), "zsh".into());
        m
    }

    #[test]
    fn render_produces_non_empty_output() {
        let intro = fixture_intro();
        let out = render_intro(&intro, &tokens());
        assert!(!out.is_empty());
    }

    #[test]
    fn text_block_substitutes_tokens() {
        let intro = fixture_intro();
        let out = render_intro(&intro, &tokens());
        assert!(out.contains("proxikal"), "token not substituted");
    }

    #[test]
    fn keyval_block_renders_keys_and_values() {
        let intro = fixture_intro();
        let out = render_intro(&intro, &tokens());
        assert!(out.contains("OS"), "key missing");
        assert!(out.contains("macOS 15"), "value missing");
    }

    #[test]
    fn separator_block_repeats_char() {
        let intro = fixture_intro();
        let out = render_intro(&intro, &tokens());
        assert!(out.contains("──────"), "separator missing");
    }

    #[test]
    fn unknown_token_resolves_to_empty() {
        let text = substitute("Hello {{unknown}}!", &HashMap::new());
        assert_eq!(text, "Hello !");
    }

    #[test]
    fn substitute_handles_multiple_tokens() {
        let mut toks = HashMap::new();
        toks.insert("a".into(), "1".into());
        toks.insert("b".into(), "2".into());
        let out = substitute("{{a}} + {{b}} = ?", &toks);
        assert_eq!(out, "1 + 2 = ?");
    }

    #[test]
    fn hex_color_produces_ansi_escape() {
        let styled = apply_style("text", Some("#ff0000"), false);
        assert!(styled.contains("\x1b[38;2;255;0;0m"));
    }

    #[test]
    fn named_color_produces_ansi_escape() {
        let styled = apply_style("text", Some("green"), false);
        assert!(styled.contains("\x1b[38;2;"));
    }

    #[test]
    fn bold_wraps_text() {
        let styled = apply_style("text", None, true);
        assert!(styled.contains("\x1b[1m"));
        assert!(styled.contains("\x1b[0m"));
    }

    #[test]
    fn no_color_no_bold_returns_plain() {
        let styled = apply_style("plain", None, false);
        assert_eq!(styled, "plain");
    }

    #[test]
    fn empty_blocks_renders_empty() {
        let intro = Intro {
            meta: IntroMeta::default(),
            display: DisplayConfig::default(),
            blocks: vec![],
        };
        let out = render_intro(&intro, &HashMap::new());
        assert!(out.is_empty());
    }
}
