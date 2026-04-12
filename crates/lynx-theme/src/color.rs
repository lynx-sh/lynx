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
    /// Named color — see `named_to_rgb` for the full registry.
    Named(String),
}

impl Color {
    /// Render this color as an ANSI foreground escape sequence, downgrading
    /// gracefully based on terminal capability.
    pub fn render_fg(&self, cap: TermCapability) -> String {
        match self {
            Color::Hex(hex) => render_hex_fg(hex, cap),
            Color::Ansi256(n) => render_256_fg(*n, cap),
            Color::Named(name) => match named_to_rgb(name) {
                Some((r, g, b)) => render_rgb_fg(r, g, b, cap),
                None => String::new(),
            },
        }
    }

    /// Render this color as an ANSI background escape sequence, downgrading
    /// gracefully based on terminal capability.
    pub fn render_bg(&self, cap: TermCapability) -> String {
        match self {
            Color::Hex(hex) => render_hex_bg(hex, cap),
            Color::Ansi256(n) => render_256_bg(*n, cap),
            Color::Named(name) => match named_to_rgb(name) {
                Some((r, g, b)) => render_rgb_bg(r, g, b, cap),
                None => String::new(),
            },
        }
    }

    /// ANSI reset sequence.
    pub fn reset() -> &'static str {
        "\x1b[0m"
    }
}

fn render_hex_fg(hex: &str, cap: TermCapability) -> String {
    match parse_hex(hex) {
        Some((r, g, b)) => render_rgb_fg(r, g, b, cap),
        None => String::new(),
    }
}

/// Render raw RGB values as an ANSI foreground escape, downgrading by capability.
pub fn render_rgb_fg(r: u8, g: u8, b: u8, cap: TermCapability) -> String {
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

fn render_hex_bg(hex: &str, cap: TermCapability) -> String {
    match parse_hex(hex) {
        Some((r, g, b)) => render_rgb_bg(r, g, b, cap),
        None => String::new(),
    }
}

/// Render raw RGB values as an ANSI background escape, downgrading by capability.
pub fn render_rgb_bg(r: u8, g: u8, b: u8, cap: TermCapability) -> String {
    match cap {
        TermCapability::TrueColor => format!("\x1b[48;2;{r};{g};{b}m"),
        TermCapability::Ansi256 => {
            let idx = rgb_to_256(r, g, b);
            format!("\x1b[48;5;{idx}m")
        }
        TermCapability::Basic16 => {
            let idx = rgb_to_16(r, g, b);
            format!("\x1b[{}m", 40 + idx)
        }
        TermCapability::None => String::new(),
    }
}

fn render_256_bg(idx: u8, cap: TermCapability) -> String {
    match cap {
        TermCapability::TrueColor | TermCapability::Ansi256 => format!("\x1b[48;5;{idx}m"),
        TermCapability::Basic16 => {
            let basic = if idx < 16 { idx } else { idx % 8 };
            format!("\x1b[{}m", 40 + basic)
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
    let ri = (r as u32 * 5 / 255) as u8;
    let gi = (g as u32 * 5 / 255) as u8;
    let bi = (b as u32 * 5 / 255) as u8;
    16 + 36 * ri + 6 * gi + bi
}

/// Approximate nearest ANSI 16 color (0–7) from RGB.
fn rgb_to_16(r: u8, g: u8, b: u8) -> u8 {
    let rv = if r > 127 { 1u8 } else { 0 };
    let gv = if g > 127 { 1u8 } else { 0 };
    let bv = if b > 127 { 1u8 } else { 0 };
    rv | (gv << 1) | (bv << 2)
}

/// Full named color registry — returns curated (r, g, b) triples.
///
/// Named colors are first-class (D-019): they resolve to curated hex values and
/// render at the best terminal capability available. They are NOT aliases for
/// ANSI 0–7 slots. "blue" on a TrueColor terminal renders as a good-looking
/// 24-bit value, not xterm index 4.
///
/// # Naming conventions
/// - Base names: `red`, `green`, `blue`, `yellow`, `magenta`, `cyan`, `white`,
///   `black`, `grey` / `gray`
/// - Light/bright variants: `light-blue`, `bright-blue` (identical)
/// - Dark variants: `dark-blue`
/// - Additional palette: `orange`, `pink`, `purple`, `teal`, `gold`, `coral`,
///   `indigo`, `lime`, `brown`, `navy`, `silver`, `sky`, `lavender`, `mint`,
///   `peach`, `rose`, `violet`, `amber`
pub fn named_to_rgb(name: &str) -> Option<(u8, u8, u8)> {
    match name {
        // ── Base colors ──────────────────────────────────────────────────────
        "black"                                  => Some((26,  27,  38)),   // #1a1b26
        "white"                                  => Some((192, 202, 245)),  // #c0caf5
        "red"                                    => Some((247, 118, 142)),  // #f7768e
        "green"                                  => Some((158, 206, 106)),  // #9ece6a
        "yellow"                                 => Some((224, 175, 104)),  // #e0af68
        "blue"                                   => Some((122, 162, 247)),  // #7aa2f7
        "magenta"                                => Some((187, 154, 247)),  // #bb9af7
        "cyan"                                   => Some((125, 207, 255)),  // #7dcfff
        "grey"          | "gray"                 => Some((86,  95,  137)),  // #565f89

        // ── Light / bright variants ───────────────────────────────────────
        "light-red"     | "bright-red"           => Some((255, 117, 127)),  // #ff757f
        "light-green"   | "bright-green"         => Some((196, 240, 127)),  // #c4f07f
        "light-yellow"  | "bright-yellow"        => Some((255, 199, 119)),  // #ffc777
        "light-blue"    | "bright-blue"          => Some((130, 170, 255)),  // #82aaff
        "light-magenta" | "bright-magenta"       => Some((215, 153, 255)),  // #d799ff
        "light-cyan"    | "bright-cyan"          => Some((134, 225, 252)),  // #86e1fc
        "light-grey"    | "light-gray"
        | "bright-grey" | "bright-gray"          => Some((115, 122, 162)),  // #737aa2
        "bright-white"                           => Some((255, 255, 255)),  // #ffffff
        "bright-black"  | "off-black"            => Some((65,  72,  104)),  // #414868

        // ── Dark variants ────────────────────────────────────────────────
        "dark-red"                               => Some((197,  59,  83)),  // #c53b53
        "dark-green"                             => Some((90,  158,  48)),  // #5a9e30
        "dark-yellow"                            => Some((184, 134,  11)),  // #b8860b
        "dark-blue"                              => Some((61,   89, 161)),  // #3d59a1
        "dark-magenta"                           => Some((147, 112, 219)),  // #9370db
        "dark-cyan"                              => Some((0,   119, 168)),  // #0077a8
        "dark-grey"     | "dark-gray"            => Some((59,   61,  87)),  // #3b3d57

        // ── Extended palette ──────────────────────────────────────────────
        "orange"                                 => Some((255, 158, 100)),  // #ff9e64
        "pink"                                   => Some((255, 121, 198)),  // #ff79c6
        "purple"                                 => Some((157, 124, 216)),  // #9d7cd8
        "teal"                                   => Some((26,  188, 156)),  // #1abc9c
        "gold"                                   => Some((230, 168,  23)),  // #e6a817
        "coral"                                  => Some((242, 139, 130)),  // #f28b82
        "indigo"                                 => Some((92,  124, 250)),  // #5c7cfa
        "lime"                                   => Some((163, 230,  53)),  // #a3e635
        "brown"                                  => Some((200, 160, 112)),  // #c8a070
        "navy"                                   => Some((30,   58,  95)),  // #1e3a5f
        "silver"                                 => Some((176, 184, 216)),  // #b0b8d8
        "sky"                                    => Some((135, 206, 235)),  // #87ceeb
        "lavender"                               => Some((181, 160, 220)),  // #b5a0dc
        "mint"                                   => Some((152, 255, 152)),  // #98ff98
        "peach"                                  => Some((255, 200, 149)),  // #ffc895
        "rose"                                   => Some((255, 130, 130)),  // #ff8282
        "violet"                                 => Some((238, 130, 238)),  // #ee82ee
        "amber"                                  => Some((255, 191,   0)),  // #ffbf00

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
    fn named_blue_truecolor_is_curated_hex_not_ansi_index() {
        // blue must render as 24-bit RGB on TrueColor — not xterm index 4.
        let c = Color::Named("blue".into());
        let s = c.render_fg(TermCapability::TrueColor);
        // named_to_rgb("blue") = (122, 162, 247)
        assert_eq!(s, "\x1b[38;2;122;162;247m");
    }

    #[test]
    fn named_colors_work_on_all_capabilities() {
        for name in &[
            "blue", "light-blue", "dark-blue", "bright-blue",
            "red", "light-red", "dark-red",
            "green", "yellow", "magenta", "cyan", "grey", "gray",
            "orange", "pink", "purple", "teal", "gold", "coral",
        ] {
            for cap in [
                TermCapability::TrueColor,
                TermCapability::Ansi256,
                TermCapability::Basic16,
            ] {
                let c = Color::Named(name.to_string());
                let s = c.render_fg(cap);
                assert!(!s.is_empty(), "named color '{name}' should produce output in {cap:?}");
            }
        }
    }

    #[test]
    fn light_blue_differs_from_blue_on_truecolor() {
        let blue = Color::Named("blue".into()).render_fg(TermCapability::TrueColor);
        let light_blue = Color::Named("light-blue".into()).render_fg(TermCapability::TrueColor);
        assert_ne!(blue, light_blue, "light-blue should have different RGB than blue");
    }

    #[test]
    fn bright_blue_is_alias_for_light_blue() {
        let bright = Color::Named("bright-blue".into()).render_fg(TermCapability::TrueColor);
        let light = Color::Named("light-blue".into()).render_fg(TermCapability::TrueColor);
        assert_eq!(bright, light);
    }

    #[test]
    fn grey_and_gray_are_aliases() {
        let grey = Color::Named("grey".into()).render_fg(TermCapability::TrueColor);
        let gray = Color::Named("gray".into()).render_fg(TermCapability::TrueColor);
        assert_eq!(grey, gray);
    }

    #[test]
    fn unknown_named_color_returns_empty() {
        let c = Color::Named("not-a-real-color".into());
        assert_eq!(c.render_fg(TermCapability::TrueColor), "");
    }

    #[test]
    fn hex_bg_truecolor() {
        let c = Color::Hex("#1a1b26".into());
        let s = c.render_bg(TermCapability::TrueColor);
        assert_eq!(s, "\x1b[48;2;26;27;38m");
    }

    #[test]
    fn hex_bg_downgrades_to_256() {
        let c = Color::Hex("#1a1b26".into());
        let s = c.render_bg(TermCapability::Ansi256);
        assert!(s.starts_with("\x1b[48;5;"));
    }

    #[test]
    fn hex_bg_downgrades_to_basic16() {
        let c = Color::Hex("#0000ff".into());
        let s = c.render_bg(TermCapability::Basic16);
        assert!(s.starts_with("\x1b[4"), "expected bg basic16 code, got: {s}");
    }

    #[test]
    fn named_blue_bg_truecolor() {
        let c = Color::Named("blue".into());
        let s = c.render_bg(TermCapability::TrueColor);
        assert_eq!(s, "\x1b[48;2;122;162;247m");
    }

    #[test]
    fn ansi256_bg() {
        let c = Color::Ansi256(33);
        let s = c.render_bg(TermCapability::Ansi256);
        assert_eq!(s, "\x1b[48;5;33m");
    }

    #[test]
    fn bg_none_cap_returns_empty() {
        let c = Color::Hex("#ffffff".into());
        assert_eq!(c.render_bg(TermCapability::None), "");
    }

    #[test]
    fn unknown_named_bg_returns_empty() {
        let c = Color::Named("not-a-color".into());
        assert_eq!(c.render_bg(TermCapability::TrueColor), "");
    }

    #[test]
    fn none_cap_returns_empty() {
        let c = Color::Hex("#ffffff".into());
        assert_eq!(c.render_fg(TermCapability::None), "");
        let c = Color::Named("blue".into());
        assert_eq!(c.render_fg(TermCapability::None), "");
    }
}
