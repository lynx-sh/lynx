//! LS_COLORS / EZA_COLORS / BSD LSCOLORS generation from theme color types.

use crate::colors::{LsColors, LsColorsEntry};

impl LsColors {
    /// Build the value string for `export LS_COLORS=...`.
    ///
    /// Format: `di=<sgr>:ln=<sgr>:ex=<sgr>:...` (colon-separated type=sgr pairs).
    /// Extension-based entries (`*.tar`, `*.jpg`, etc.) are appended for archive,
    /// image, and audio categories.
    ///
    /// Returns `None` when the `[ls_colors]` table is entirely absent (all fields None).
    pub fn to_ls_colors_string(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();

        if let Some(e) = &self.dir {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("di={sgr}"));
            }
        }
        if let Some(e) = &self.symlink {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("ln={sgr}"));
            }
        }
        if let Some(e) = &self.executable {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("ex={sgr}"));
            }
        }
        if let Some(e) = &self.broken {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("or={sgr}"));
            }
        }
        if let Some(e) = &self.other_writable {
            if let Some(sgr) = entry_sgr(e) {
                parts.push(format!("ow={sgr}"));
            }
        }
        if let Some(e) = &self.archive {
            if let Some(sgr) = entry_sgr(e) {
                for ext in &[
                    "tar", "gz", "bz2", "xz", "zip", "7z", "rar", "tgz", "zst", "lz4",
                ] {
                    parts.push(format!("*.{ext}={sgr}"));
                }
            }
        }
        if let Some(e) = &self.image {
            if let Some(sgr) = entry_sgr(e) {
                for ext in &[
                    "jpg", "jpeg", "png", "gif", "bmp", "svg", "webp", "ico", "tiff", "tif",
                ] {
                    parts.push(format!("*.{ext}={sgr}"));
                }
            }
        }
        if let Some(e) = &self.audio {
            if let Some(sgr) = entry_sgr(e) {
                for ext in &["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus", "wma"] {
                    parts.push(format!("*.{ext}={sgr}"));
                }
            }
        }

        // Per-extension overrides — these come AFTER category entries so they
        // take priority (LS_COLORS uses last-match-wins for duplicate extensions).
        let mut ext_keys: Vec<&String> = self.extensions.keys().collect();
        ext_keys.sort(); // deterministic output
        for ext in ext_keys {
            if let Some(sgr) = entry_sgr(&self.extensions[ext]) {
                parts.push(format!("*.{ext}={sgr}"));
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(":"))
        }
    }

    /// Build the value string for `export EZA_COLORS=...`.
    ///
    /// Starts with the file-type entries (same as LS_COLORS), then appends
    /// eza-specific column color keys from `[ls_colors.columns]`.
    /// Eza reads LS_COLORS first, then EZA_COLORS as overrides — so emitting
    /// both here is correct and intentional.
    pub fn to_eza_colors_string(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();

        // File-type entries (shared with LS_COLORS).
        if let Some(base) = self.to_ls_colors_string() {
            parts.push(base);
        }

        // Column-specific entries — only eza reads these keys.
        let c = &self.columns;
        let col_pairs: &[(&str, &Option<String>)] = &[
            ("da", &c.date),
            ("sn", &c.size_number),
            ("sb", &c.size_unit),
            ("uu", &c.user_you),
            ("un", &c.user_other),
            ("gu", &c.group_you),
            ("gn", &c.group_other),
            ("hd", &c.header),
            ("lp", &c.symlink_path),
        ];
        for (key, val) in col_pairs {
            if let Some(color) = val {
                if let Some(sgr) = color_to_fg_sgr(color) {
                    parts.push(format!("{key}={sgr}"));
                }
            }
        }

        // Permission bits — one theme color fans out to three eza keys each.
        if let Some(color) = &c.perm_read {
            if let Some(sgr) = color_to_fg_sgr(color) {
                for key in &["ur", "gr", "or"] {
                    parts.push(format!("{key}={sgr}"));
                }
            }
        }
        if let Some(color) = &c.perm_write {
            if let Some(sgr) = color_to_fg_sgr(color) {
                for key in &["uw", "gw", "ow"] {
                    parts.push(format!("{key}={sgr}"));
                }
            }
        }
        if let Some(color) = &c.perm_exec {
            if let Some(sgr) = color_to_fg_sgr(color) {
                for key in &["ux", "gx", "ox"] {
                    parts.push(format!("{key}={sgr}"));
                }
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(":"))
        }
    }

    /// Build the BSD `LSCOLORS` string for macOS `/bin/ls`.
    ///
    /// BSD format: 11 pairs of characters (fg+bg) for fixed file types:
    /// directory, symlink, socket, pipe, executable, block-special,
    /// char-special, setuid, setgid, other-writable+sticky, other-writable.
    ///
    /// Character codes: a=black b=red c=green d=brown/yellow e=blue f=magenta
    /// g=cyan h=white x=default. Uppercase = bold.
    pub fn to_bsd_lscolors(&self) -> String {
        let mut s = String::with_capacity(22);
        // 1: directory
        s.push_str(&bsd_pair(self.dir.as_ref()));
        // 2: symlink
        s.push_str(&bsd_pair(self.symlink.as_ref()));
        // 3: socket (no theme mapping — default)
        s.push_str("xx");
        // 4: pipe (no theme mapping — default)
        s.push_str("xx");
        // 5: executable
        s.push_str(&bsd_pair(self.executable.as_ref()));
        // 6: block special — default
        s.push_str("xx");
        // 7: char special — default
        s.push_str("xx");
        // 8: setuid exe — default
        s.push_str("xx");
        // 9: setgid exe — default
        s.push_str("xx");
        // 10: other-writable+sticky — default
        s.push_str("xx");
        // 11: other-writable
        s.push_str(&bsd_pair(self.other_writable.as_ref()));
        s
    }
}

/// Convert an `LsColorsEntry` to an ANSI SGR parameter string (e.g. `"1;34"`).
/// Returns `None` if the entry has no color information.
fn entry_sgr(e: &LsColorsEntry) -> Option<String> {
    let mut codes: Vec<String> = Vec::new();

    if e.bold {
        codes.push("1".to_string());
    }
    if let Some(fg) = &e.fg {
        if let Some(sgr) = color_to_fg_sgr(fg) {
            codes.push(sgr);
        }
    }
    if let Some(bg) = &e.bg {
        if let Some(sgr) = color_to_bg_sgr(bg) {
            codes.push(sgr);
        }
    }

    if codes.is_empty() {
        None
    } else {
        Some(codes.join(";"))
    }
}

/// Map an `LsColorsEntry` to a BSD LSCOLORS fg+bg pair (2 chars).
/// BSD codes: a=black b=red c=green d=brown e=blue f=magenta g=cyan h=white x=default.
/// Uppercase = bold variant.
fn bsd_pair(entry: Option<&LsColorsEntry>) -> String {
    let Some(e) = entry else {
        return "xx".to_string();
    };
    let fg = e
        .fg
        .as_deref()
        .and_then(|c| resolve_color_rgb(c))
        .map(|(r, g, b)| rgb_to_bsd_char(r, g, b, e.bold))
        .unwrap_or('x');
    let bg = e
        .bg
        .as_deref()
        .and_then(|c| resolve_color_rgb(c))
        .map(|(r, g, b)| rgb_to_bsd_char(r, g, b, false))
        .unwrap_or('x');
    format!("{fg}{bg}")
}

/// Map an RGB color to the nearest BSD LSCOLORS character.
///
/// Uses a hue-first approach: identify the dominant channel(s) to pick the
/// correct ANSI hue, then use brightness to decide if it's a chromatic color
/// or achromatic (black/white). This avoids the Euclidean RGB trap where
/// light blues map to white instead of blue.
fn rgb_to_bsd_char(r: u8, g: u8, b: u8, bold: bool) -> char {
    let (ri, gi, bi) = (r as i32, g as i32, b as i32);
    let max = ri.max(gi).max(bi);
    let min = ri.min(gi).min(bi);
    let chroma = max - min;

    // Achromatic: if saturation is very low, map to black or white by brightness.
    let ch = if chroma < 30 {
        if max < 80 { 'a' } else { 'h' } // black or white
    } else {
        // Chromatic: pick hue based on dominant channel(s).
        match (ri == max, gi == max, bi == max) {
            // Red dominant
            (true, false, false) => {
                if gi > bi + 40 { 'd' } // red+green lean → brown/yellow
                else if bi > gi + 40 { 'f' } // red+blue lean → magenta
                else { 'b' } // pure red
            }
            // Green dominant
            (false, true, false) => {
                if bi > ri + 40 { 'g' } // green+blue lean → cyan
                else if ri > bi + 40 { 'd' } // green+red lean → yellow/brown
                else { 'c' } // pure green
            }
            // Blue dominant
            (false, false, true) => {
                if ri > gi + 40 { 'f' } // blue+red lean → magenta
                else if gi > ri + 40 { 'g' } // blue+green lean → cyan
                else { 'e' } // pure blue
            }
            // Ties — secondary channel decides
            (true, true, false) => 'd',  // red+green = yellow/brown
            (true, false, true) => 'f',  // red+blue = magenta
            (false, true, true) => 'g',  // green+blue = cyan
            (true, true, true) => 'h',   // all equal = white
            _ => 'h',
        }
    };

    if bold { ch.to_ascii_uppercase() } else { ch }
}

/// Convert a color string (named or hex) to a truecolor (24-bit) fg SGR parameter.
/// Uses `38;2;R;G;B` format — supported by all modern terminals (iTerm2, kitty,
/// Alacritty, WezTerm, Windows Terminal, GNOME Terminal, etc.).
fn color_to_fg_sgr(color: &str) -> Option<String> {
    let (r, g, b) = resolve_color_rgb(color)?;
    Some(format!("38;2;{r};{g};{b}"))
}

/// Convert a color string (named or hex) to a truecolor (24-bit) bg SGR parameter.
fn color_to_bg_sgr(color: &str) -> Option<String> {
    let (r, g, b) = resolve_color_rgb(color)?;
    Some(format!("48;2;{r};{g};{b}"))
}

/// Resolve a color string to (r, g, b). Handles hex (#rrggbb) and named colors.
fn resolve_color_rgb(color: &str) -> Option<(u8, u8, u8)> {
    if color.starts_with('#') {
        parse_hex_rgb(color)
    } else {
        crate::color::named_to_rgb(color)
    }
}

fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let s = hex.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors::LsColorsEntry;

    fn blue_dir_entry() -> LsColorsEntry {
        LsColorsEntry {
            fg: Some("blue".to_string()),
            bg: None,
            bold: true,
        }
    }

    #[test]
    fn empty_ls_colors_returns_none() {
        let lsc = LsColors::default();
        assert!(lsc.to_ls_colors_string().is_none());
    }

    #[test]
    fn dir_entry_produces_di_key() {
        let lsc = LsColors {
            dir: Some(blue_dir_entry()),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.starts_with("di="), "expected di= prefix, got: {s}");
        // bold + truecolor fg code
        assert!(s.contains("1;38;2;"), "expected bold+truecolor code in: {s}");
    }

    #[test]
    fn archive_entry_expands_to_extensions() {
        let lsc = LsColors {
            archive: Some(LsColorsEntry {
                fg: Some("red".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.contains("*.tar="), "expected *.tar in: {s}");
        assert!(s.contains("*.zip="), "expected *.zip in: {s}");
        assert!(s.contains("*.gz="), "expected *.gz in: {s}");
    }

    #[test]
    fn image_entry_expands_to_extensions() {
        let lsc = LsColors {
            image: Some(LsColorsEntry {
                fg: Some("magenta".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.contains("*.jpg="), "expected *.jpg in: {s}");
        assert!(s.contains("*.png="), "expected *.png in: {s}");
    }

    #[test]
    fn hex_color_is_truecolor() {
        let lsc = LsColors {
            dir: Some(LsColorsEntry {
                fg: Some("#7aa2f7".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.contains("38;2;122;162;247"), "expected truecolor code in: {s}");
    }

    #[test]
    fn bg_color_produces_truecolor_48_code() {
        let lsc = LsColors {
            dir: Some(LsColorsEntry {
                fg: Some("blue".to_string()),
                bg: Some("green".to_string()),
                bold: false,
            }),
            ..Default::default()
        };
        let s = lsc.to_ls_colors_string().unwrap();
        assert!(s.contains("48;2;"), "expected bg truecolor code in: {s}");
    }

    #[test]
    fn eza_colors_matches_ls_colors() {
        let lsc = LsColors {
            dir: Some(blue_dir_entry()),
            ..Default::default()
        };
        assert_eq!(lsc.to_ls_colors_string(), lsc.to_eza_colors_string());
    }

    #[test]
    fn inline_toml_ls_colors_parses() {
        let toml = r#"
[meta]
name = "test"

[segments.left]
order = []

[ls_colors]
dir = { fg = "blue", bold = true }
"#;
        let theme: crate::schema::Theme = toml::from_str(toml).expect("should parse");
        assert!(theme.ls_colors.dir.is_some(), "dir should be Some after parse");
        let s = theme.ls_colors.to_ls_colors_string().unwrap();
        assert!(s.contains("di="), "expected di= in: {s}");
    }

    #[test]
    fn extension_colors_emitted() {
        let toml = r##"
[meta]
name = "test"

[segments.left]
order = []

[ls_colors]
dir = { fg = "blue", bold = true }

[ls_colors.extensions]
rs = { fg = "#e7894f" }
py = { fg = "#4584b6" }
sh = { fg = "#e0af68", bold = true }
"##;
        let theme: crate::schema::Theme = toml::from_str(toml).expect("should parse");
        assert_eq!(theme.ls_colors.extensions.len(), 3);
        let s = theme.ls_colors.to_ls_colors_string().unwrap();
        assert!(s.contains("*.rs=38;2;231;137;79"), "missing rs ext in: {s}");
        assert!(s.contains("*.py=38;2;69;132;182"), "missing py ext in: {s}");
        assert!(s.contains("*.sh=1;38;2;224;175;104"), "missing sh ext in: {s}");
    }

    #[test]
    fn tokyo_night_extensions_roundtrip() {
        let theme = crate::loader::parse_and_validate(
            include_str!("../../../themes/tokyo-night.toml"), "tokyo-night"
        ).unwrap();
        assert!(!theme.ls_colors.extensions.is_empty(), "extensions should be non-empty");
        let s = theme.ls_colors.to_ls_colors_string().unwrap();
        assert!(s.contains("*.rs="), "missing *.rs in: {s}");
        assert!(s.contains("*.py="), "missing *.py in: {s}");
        assert!(s.contains("*.sh="), "missing *.sh in: {s}");
        assert!(s.contains("*.toml="), "missing *.toml in: {s}");
    }

    #[test]
    fn default_theme_ls_colors_is_non_empty() {
        let theme = crate::loader::parse_and_validate(
            include_str!("../../../themes/default.toml"), "default"
        ).unwrap();
        assert!(
            theme.ls_colors.to_ls_colors_string().is_some(),
            "default theme should have non-empty ls_colors; dir={:?}", theme.ls_colors.dir
        );
    }

    #[test]
    fn minimal_theme_ls_colors_is_non_empty() {
        let theme = crate::loader::parse_and_validate(
            include_str!("../../../themes/minimal.toml"), "minimal"
        ).unwrap();
        assert!(
            theme.ls_colors.to_ls_colors_string().is_some(),
            "minimal theme should have non-empty ls_colors"
        );
    }
}
