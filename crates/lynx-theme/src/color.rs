use serde::{Deserialize, Serialize};

use crate::terminal::TermCapability;

/// A color value that can be specified in a theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Color {
    /// 24-bit hex color, e.g. "#7aa2f7".
    Hex(String),
    /// xterm-256 index (0–255).
    Ansi256(u8),
    /// Named color: blue, red, green, yellow, grey, magenta, cyan, white, black.
    Named(String),
}

impl Color {
    /// Render this color as an ANSI foreground escape sequence, downgrading
    /// gracefully based on terminal capability.
    pub fn render_fg(&self, cap: TermCapability) -> String {
        match self {
            Color::Hex(hex) => render_hex_fg(hex, cap),
            Color::Ansi256(n) => render_256_fg(*n, cap),
            Color::Named(name) => {
                if let Some(idx) = named_to_256(name) {
                    render_256_fg(idx, cap)
                } else {
                    String::new()
                }
            }
        }
    }

    /// ANSI reset sequence.
    pub fn reset() -> &'static str {
        "\x1b[0m"
    }
}

fn render_hex_fg(hex: &str, cap: TermCapability) -> String {
    let (r, g, b) = match parse_hex(hex) {
        Some(v) => v,
        None => return String::new(),
    };
    match cap {
        TermCapability::TrueColor => format!("\x1b[38;2;{r};{g};{b}m"),
        TermCapability::Ansi256 => {
            let idx = rgb_to_256(r, g, b);
            format!("\x1b[38;5;{idx}m")
        }
        TermCapability::Basic16 => {
            let idx = rgb_to_16(r, g, b);
            format!("\x1b[{}m", 30 + idx)
        }
        TermCapability::None => String::new(),
    }
}

fn render_256_fg(idx: u8, cap: TermCapability) -> String {
    match cap {
        TermCapability::TrueColor | TermCapability::Ansi256 => format!("\x1b[38;5;{idx}m"),
        TermCapability::Basic16 => {
            let basic = if idx < 16 { idx } else { idx % 8 };
            format!("\x1b[{}m", 30 + basic)
        }
        TermCapability::None => String::new(),
    }
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let s = hex.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Nearest xterm-256 color cube index for an RGB value.
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    // Use the 6x6x6 color cube (indices 16–231).
    let ri = (r as u32 * 5 / 255) as u8;
    let gi = (g as u32 * 5 / 255) as u8;
    let bi = (b as u32 * 5 / 255) as u8;
    16 + 36 * ri + 6 * gi + bi
}

/// Approximate nearest ANSI 16 color (0–7) from RGB.
fn rgb_to_16(r: u8, g: u8, b: u8) -> u8 {
    // Map to 3-bit RGB.
    let rv = if r > 127 { 1u8 } else { 0 };
    let gv = if g > 127 { 1u8 } else { 0 };
    let bv = if b > 127 { 1u8 } else { 0 };
    rv | (gv << 1) | (bv << 2)
}

fn named_to_256(name: &str) -> Option<u8> {
    match name {
        "black"   => Some(0),
        "red"     => Some(1),
        "green"   => Some(2),
        "yellow"  => Some(3),
        "blue"    => Some(4),
        "magenta" => Some(5),
        "cyan"    => Some(6),
        "white"   => Some(7),
        "grey" | "gray" => Some(8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_truecolor() {
        let c = Color::Hex("#7aa2f7".into());
        let s = c.render_fg(TermCapability::TrueColor);
        assert_eq!(s, "\x1b[38;2;122;162;247m");
    }

    #[test]
    fn hex_downgrades_to_256() {
        let c = Color::Hex("#7aa2f7".into());
        let s = c.render_fg(TermCapability::Ansi256);
        assert!(s.starts_with("\x1b[38;5;"));
    }

    #[test]
    fn named_blue_all_modes() {
        for cap in [TermCapability::TrueColor, TermCapability::Ansi256, TermCapability::Basic16] {
            let c = Color::Named("blue".into());
            let s = c.render_fg(cap);
            assert!(!s.is_empty(), "named blue should produce output in {cap:?}");
        }
    }

    #[test]
    fn none_cap_returns_empty() {
        let c = Color::Hex("#ffffff".into());
        assert_eq!(c.render_fg(TermCapability::None), "");
    }
}
