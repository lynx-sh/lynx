use lynx_core::error::LynxError;
use std::io::Read as _;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context as _, Result};

/// Check if a character is a Nerd Font glyph (Powerline, Font Awesome,
/// Devicons, Seti-UI, Material Design, Weather, Codicons, or any other
/// icon set bundled in Nerd Fonts). Covers the full Private Use Area
/// ranges that Nerd Fonts patches into.
fn is_nerd_font_glyph(ch: char) -> bool {
    let cp = ch as u32;
    // BMP Private Use Area icon ranges (all bundled in Nerd Fonts):
    //   U+E000–U+E00A  Pomicons
    //   U+E0A0–U+E0D7  Powerline + Powerline Extra
    //   U+E200–U+E2FF  Seti-UI / Custom
    //   U+E300–U+E3FF  Seti-UI Extended
    //   U+E5FA–U+E6AC  Seti-UI + Custom
    //   U+E600–U+E6FF  Seti-UI Extended (overlaps above)
    //   U+E700–U+E7FF  Devicons
    //   U+EA60–U+EBEB  Codicons
    //   U+F000–U+F2FF  Font Awesome
    //   U+F300–U+F4FF  Font Awesome Extension
    //   U+F500–U+F8FF  Material Design Icons
    // Supplementary Private Use Area (Nerd Font v3+):
    //   U+F0001–U+F1AF0  Nerd Font Symbols (MDI extended)
    //
    // Rather than enumerate every sub-range, cover the full PUA blocks:
    (0xE000..=0xF8FF).contains(&cp)          // BMP Private Use Area
        || (0xF0001..=0xF1AF0).contains(&cp) // Supplementary PUA (Nerd Font v3)
}

/// Check if a string contains any Nerd Font glyphs.
fn has_nerd_glyphs(s: &str) -> bool {
    s.chars().any(is_nerd_font_glyph)
}

/// Check if a theme uses any Nerd Font / icon font glyphs anywhere —
/// separators, segment icons, leading/trailing chars, text content,
/// transient prompt, or filler.
pub fn theme_needs_nerd_font(theme: &lynx_theme::Theme) -> bool {
    // 1. Global separators.
    let sep = &theme.separators;
    for opt in [
        &sep.left.char,
        &sep.right.char,
        &sep.left_edge.char,
        &sep.right_edge.char,
    ] {
        if opt.as_ref().is_some_and(|s| has_nerd_glyphs(s)) {
            return true;
        }
    }

    // 2. Transient prompt template.
    if let Some(ref t) = theme.transient {
        if has_nerd_glyphs(&t.template) {
            return true;
        }
    }

    // 3. Filler character.
    if let Some(ref f) = theme.segments.filler {
        if has_nerd_glyphs(&f.char) {
            return true;
        }
    }

    // 4. Per-segment config — scan all string values recursively.
    for value in theme.segment.values() {
        if toml_value_has_nerd_glyphs(value) {
            return true;
        }
    }

    false
}

/// Recursively scan a TOML value tree for Nerd Font glyphs in any string.
fn toml_value_has_nerd_glyphs(value: &toml::Value) -> bool {
    match value {
        toml::Value::String(s) => has_nerd_glyphs(s),
        toml::Value::Array(arr) => arr.iter().any(toml_value_has_nerd_glyphs),
        toml::Value::Table(tbl) => tbl.values().any(toml_value_has_nerd_glyphs),
        _ => false,
    }
}

/// Find installed Nerd Font PostScript base names (e.g. "JetBrainsMonoNLNF").
/// On macOS, queries the font system for real PostScript names.
/// Falls back to filename-based extraction on other platforms.
pub fn find_installed_nerd_fonts() -> Vec<String> {
    let home = lynx_core::paths::home();
    let font_dirs: Vec<PathBuf> = {
        vec![
            home.join("Library/Fonts"),
            "/Library/Fonts".into(),
            "/System/Library/Fonts".into(),
            home.join(".local/share/fonts"),
            home.join(".fonts"),
            "/usr/share/fonts".into(),
            "/usr/local/share/fonts".into(),
        ]
    };

    // Collect Nerd Font file paths first.
    let mut nerd_font_files = Vec::new();
    for dir in &font_dirs {
        scan_dir_for_nerd_font_files(dir, &mut nerd_font_files);
    }

    // Resolve PostScript names from font files.
    let mut families = std::collections::HashSet::new();
    for path in &nerd_font_files {
        if let Some(ps_base) = postscript_base_name(path) {
            families.insert(ps_base);
        }
    }

    // Fallback: if we found font files but couldn't resolve any PostScript names,
    // extract from filenames (less reliable but better than nothing).
    if families.is_empty() && !nerd_font_files.is_empty() {
        for path in &nerd_font_files {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                // "JetBrainsMonoNerdFont-Regular" → "JetBrainsMonoNerdFont"
                if let Some(family) = stem.split('-').next() {
                    families.insert(family.to_string());
                }
            }
        }
    }

    let mut result: Vec<String> = families.into_iter().collect();
    result.sort();
    result
}

/// Collect paths of Nerd Font files (*.ttf, *.otf with "nerd" in the name).
fn scan_dir_for_nerd_font_files(dir: &PathBuf, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir_for_nerd_font_files(&path, files);
        } else {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains("nerd") && (name.ends_with(".ttf") || name.ends_with(".otf")) {
                // Only collect Regular weight to avoid duplicates per family.
                if name.contains("regular") {
                    files.push(path);
                }
            }
        }
    }
}

/// Get the PostScript base name (without style suffix) for a font file.
/// e.g. "JetBrainsMonoNLNF" from a file whose PostScript name is "JetBrainsMonoNLNF-Regular".
fn postscript_base_name(font_path: &PathBuf) -> Option<String> {
    if cfg!(target_os = "macos") {
        postscript_name_macos(font_path)
    } else {
        postscript_name_fc(font_path)
    }
}

/// macOS: use mdls to query the font's PostScript name.
fn postscript_name_macos(font_path: &PathBuf) -> Option<String> {
    let output = Command::new("mdls")
        .args(["-name", "com_apple_ats_name_postscript", "-raw"])
        .arg(font_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // mdls -raw can return "(null)", a bare string, or a plist array like:
    //   (\n    "JetBrainsMonoNLNF-Regular"\n)
    // Extract the first quoted string, or use the whole trimmed value.
    let raw = text
        .lines()
        .filter_map(|l| {
            let t = l.trim().trim_matches('"');
            if t.is_empty() || t == "(" || t == ")" || t == "(null)" {
                None
            } else {
                Some(t.to_string())
            }
        })
        .next()?;
    // PostScript name is e.g. "JetBrainsMonoNLNF-Regular" — strip the style suffix.
    Some(raw.split('-').next().unwrap_or(&raw).to_string())
}

/// Linux: use fc-query to get the PostScript name.
fn postscript_name_fc(font_path: &PathBuf) -> Option<String> {
    let output = Command::new("fc-query")
        .args(["--format", "%{postscriptname}"])
        .arg(font_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    Some(raw.split('-').next().unwrap_or(&raw).to_string())
}

/// Detect the current terminal emulator.
enum Terminal {
    ITerm2,
    Other,
}

fn detect_terminal() -> Terminal {
    if std::env::var("ITERM_SESSION_ID").is_ok()
        || std::env::var("TERM_PROGRAM").as_deref() == Ok("iTerm.app")
    {
        Terminal::ITerm2
    } else {
        Terminal::Other
    }
}

/// Check if the terminal is currently using a Nerd Font.
pub fn terminal_using_nerd_font() -> bool {
    match detect_terminal() {
        Terminal::ITerm2 => iterm2_current_font_is_nerd(),
        Terminal::Other => false, // Can't detect — assume no.
    }
}

/// Read iTerm2's current font from preferences.
fn iterm2_current_font() -> Option<String> {
    let output = Command::new("defaults")
        .args(["read", "com.googlecode.iterm2", "New Bookmarks"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // Parse the plist text output for "Normal Font" = "FontName Size";
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"Normal Font\"") {
            // "Normal Font" = "Monaco 12";
            let val = trimmed
                .split('=')
                .nth(1)?
                .trim()
                .trim_matches(';')
                .trim()
                .trim_matches('"');
            return Some(val.to_string());
        }
    }
    None
}

fn iterm2_current_font_is_nerd() -> bool {
    iterm2_current_font()
        .map(|f| {
            let lower = f.to_lowercase();
            // Check for Nerd Font indicators in both filename-style
            // names (e.g. "JetBrainsMonoNerdFont") and PostScript-style
            // names (e.g. "JetBrainsMonoNLNF-Regular" where NF/NFM/NFP = Nerd Font).
            lower.contains("nerd")
                || lower.contains("powerline")
                || lower.contains("nf-")
                || lower.contains("nfm-")
                || lower.contains("nfp-")
        })
        .unwrap_or(false)
}

/// Configure iTerm2 to use a Nerd Font.
pub fn configure_iterm2_font(font_family: &str, size: u32) -> Result<()> {
    // iTerm2 font format: "FontPostScriptName SIZE"
    // For Nerd Fonts the PostScript name is typically "FamilyName-Regular"
    let font_value = format!("{font_family}-Regular {size}");

    // Use PlistBuddy to update the Default profile's font.
    let plist = lynx_core::paths::home().join("Library/Preferences/com.googlecode.iterm2.plist");

    // Update Normal Font in the first (default) profile.
    let status = Command::new("/usr/libexec/PlistBuddy")
        .args([
            "-c",
            &format!("Set ':New Bookmarks':0:'Normal Font' '{font_value}'"),
            plist.to_str().unwrap_or_default(),
        ])
        .status()
        .context("failed to run PlistBuddy")?;

    if !status.success() {
        return Err(LynxError::Shell("PlistBuddy failed to update iTerm2 font".into()).into());
    }

    // Tell iTerm2 to reload preferences.
    // Best-effort: secondary iTerm2 pref write, non-critical
    let _ = Command::new("defaults")
        .args(["read", "com.googlecode.iterm2"])
        .output();

    println!("  ✓ iTerm2 font set to: {font_value}");
    println!("  → Restart iTerm2 or open a new tab to see the change.");
    Ok(())
}

/// Download and install a Nerd Font (FiraCode by default).
pub fn install_nerd_font() -> Result<String> {
    let font_name = "FiraCode";
    let filename_family = "FiraCodeNerdFont";
    let url =
        format!("https://github.com/ryanoasis/nerd-fonts/releases/latest/download/{font_name}.zip");

    println!("  downloading {font_name} Nerd Font...");

    let resp = ureq::get(&url)
        .call()
        .with_context(|| format!("failed to download font from {url}"))?;

    let mut bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .context("failed to read font download")?;

    let font_dir = if cfg!(target_os = "macos") {
        lynx_core::paths::home().join("Library/Fonts")
    } else {
        lynx_core::paths::home().join(".local/share/fonts")
    };
    std::fs::create_dir_all(&font_dir)?;

    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("failed to open font zip archive")?;

    let mut installed = 0;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name.ends_with(".ttf") || name.ends_with(".otf") {
            let file_name = std::path::Path::new(&name)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let dest = font_dir.join(&file_name);
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut file, &mut out)?;
            installed += 1;
        }
    }

    if cfg!(target_os = "linux") {
        // Advisory font cache refresh — failure is non-critical
        let _ = Command::new("fc-cache").arg("-f").status();
    }

    println!(
        "  ✓ installed {installed} font files to {}",
        font_dir.display()
    );

    // Resolve the real PostScript base name from the installed Regular font file.
    let regular_file = font_dir.join(format!("{filename_family}-Regular.ttf"));
    let family = postscript_base_name(&regular_file).unwrap_or_else(|| filename_family.to_string());
    Ok(family)
}

/// Ensure a Nerd Font is installed AND the terminal is configured to use it.
/// Returns true if ready to proceed, false if user chose to cancel.
pub fn ensure_nerd_font_ready() -> anyhow::Result<bool> {
    use anyhow::Context as _;

    let fonts = find_installed_nerd_fonts();
    let terminal_ok = terminal_using_nerd_font();

    if terminal_ok {
        return Ok(true);
    }

    if fonts.is_empty() {
        println!("⚠ This theme uses powerline glyphs that require a Nerd Font.");
        println!("  Without one, separator characters will render as □ or ?.");
        println!();
        print!("  Download and install a Nerd Font? [y]es / [n]o / [s]kip: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let choice = read_line_lower()?;
        match choice.as_str() {
            "y" | "yes" => {
                let family = install_nerd_font().context("font installation failed")?;
                return offer_terminal_config(&family);
            }
            "s" | "skip" => return Ok(true),
            _ => return Ok(false),
        }
    }

    let first = &fonts[0];
    println!("⚠ Nerd Font found ({first}) but your terminal isn't using it.");
    println!("  Powerline glyphs will render as □ until the terminal font is changed.");
    println!();

    offer_terminal_config(first)
}

fn offer_terminal_config(font_family: &str) -> anyhow::Result<bool> {
    if std::env::var("ITERM_SESSION_ID").is_ok()
        || std::env::var("TERM_PROGRAM").as_deref() == Ok("iTerm.app")
    {
        print!("  Configure iTerm2 to use {font_family}? [y]es / [n]o: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let choice = read_line_lower()?;
        if choice.starts_with('y') {
            let size = current_iterm2_font_size().unwrap_or(12);
            configure_iterm2_font(font_family, size)?;
            return Ok(true);
        }
        println!("  → iTerm2: Settings → Profiles → Text → Font → \"{font_family}\"");
    } else {
        println!("  → Set your terminal font to \"{font_family}\" in terminal preferences.");
    }

    print!("  Continue setting theme? [y/n]: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let choice = read_line_lower()?;
    Ok(choice.starts_with('y'))
}

fn read_line_lower() -> anyhow::Result<String> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_lowercase())
}

fn current_iterm2_font_size() -> Option<u32> {
    let output = Command::new("defaults")
        .args(["read", "com.googlecode.iterm2", "New Bookmarks"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"Normal Font\"") {
            let val = trimmed
                .split('=')
                .nth(1)?
                .trim()
                .trim_matches(';')
                .trim()
                .trim_matches('"');
            let size_str = val.split_whitespace().last()?;
            return size_str.parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_powerline_glyphs() {
        assert!(is_nerd_font_glyph('\u{e0b0}')); // Powerline right arrow
        assert!(is_nerd_font_glyph('\u{e0b6}')); // Powerline left half-circle
        assert!(is_nerd_font_glyph('\u{e0d4}')); // Powerline extra
    }

    #[test]
    fn detects_font_awesome() {
        assert!(is_nerd_font_glyph('\u{f044}')); // FA pencil
        assert!(is_nerd_font_glyph('\u{f07b}')); // FA folder
        assert!(is_nerd_font_glyph('\u{f295}')); // FA map-pin
    }

    #[test]
    fn detects_devicons() {
        assert!(is_nerd_font_glyph('\u{e718}')); // Node.js
        assert!(is_nerd_font_glyph('\u{e791}')); // Ruby
        assert!(is_nerd_font_glyph('\u{e7a8}')); // Rust
    }

    #[test]
    fn detects_material_design() {
        assert!(is_nerd_font_glyph('\u{f500}')); // MDI range start
        assert!(is_nerd_font_glyph('\u{f8ff}')); // MDI range end
    }

    #[test]
    fn detects_codicons() {
        assert!(is_nerd_font_glyph('\u{ea6c}')); // Codicon
        assert!(is_nerd_font_glyph('\u{eb99}')); // Codicon
    }

    #[test]
    fn detects_supplementary_plane() {
        assert!(is_nerd_font_glyph('\u{f0001}')); // Nerd Font v3 MDI extended
        assert!(is_nerd_font_glyph('\u{f10fe}')); // Nerd Font v3 symbol
    }

    #[test]
    fn ignores_normal_text() {
        assert!(!is_nerd_font_glyph('A'));
        assert!(!is_nerd_font_glyph('❯'));
        assert!(!is_nerd_font_glyph('─'));
        assert!(!is_nerd_font_glyph('╰'));
    }

    #[test]
    fn has_nerd_glyphs_in_string() {
        assert!(has_nerd_glyphs("\u{e0b0} hello"));
        assert!(has_nerd_glyphs("icon: \u{f07b}"));
        assert!(!has_nerd_glyphs("plain text ❯"));
        assert!(!has_nerd_glyphs(""));
    }

    #[test]
    fn toml_value_scan_finds_nested_glyphs() {
        let val = toml::Value::Table({
            let mut t = toml::map::Map::new();
            t.insert("icon".into(), toml::Value::String("\u{e718}".into()));
            t
        });
        assert!(toml_value_has_nerd_glyphs(&val));

        let plain = toml::Value::String("no icons".into());
        assert!(!toml_value_has_nerd_glyphs(&plain));
    }
}
