use std::io::Read as _;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context as _, Result};

/// Check if a theme uses Powerline / Nerd Font glyphs in its separators.
pub fn theme_needs_nerd_font(theme: &lynx_theme::Theme) -> bool {
    let check_glyph = |s: &Option<String>| -> bool {
        s.as_ref()
            .map(|c| {
                c.chars().any(|ch| {
                    let cp = ch as u32;
                    // Powerline glyphs: U+E0A0–U+E0D4
                    // Nerd Font extras: U+E200–U+E2FF, U+F000–U+F2FF
                    (0xE0A0..=0xE0D4).contains(&cp)
                        || (0xE200..=0xE2FF).contains(&cp)
                        || (0xF000..=0xF2FF).contains(&cp)
                })
            })
            .unwrap_or(false)
    };
    check_glyph(&theme.separators.left.char)
        || check_glyph(&theme.separators.right.char)
        || check_glyph(&theme.separators.left_edge.char)
        || check_glyph(&theme.separators.right_edge.char)
}

/// Find installed Nerd Font PostScript base names (e.g. "JetBrainsMonoNLNF").
/// On macOS, queries the font system for real PostScript names.
/// Falls back to filename-based extraction on other platforms.
pub fn find_installed_nerd_fonts() -> Vec<String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let font_dirs: Vec<PathBuf> = {
        let mut dirs = Vec::new();
        if let Some(ref h) = home {
            dirs.push(h.join("Library/Fonts"));
        }
        dirs.push("/Library/Fonts".into());
        dirs.push("/System/Library/Fonts".into());
        if let Some(ref h) = home {
            dirs.push(h.join(".local/share/fonts"));
            dirs.push(h.join(".fonts"));
        }
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        dirs
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
    let Ok(entries) = std::fs::read_dir(dir) else { return };
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
    if std::env::var("ITERM_SESSION_ID").is_ok() || std::env::var("TERM_PROGRAM").as_deref() == Ok("iTerm.app")
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
            let val = trimmed.split('=').nth(1)?.trim().trim_matches(';').trim().trim_matches('"');
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
    let plist = format!(
        "{}/Library/Preferences/com.googlecode.iterm2.plist",
        std::env::var("HOME").context("HOME not set")?
    );

    // Update Normal Font in the first (default) profile.
    let status = Command::new("/usr/libexec/PlistBuddy")
        .args([
            "-c",
            &format!("Set ':New Bookmarks':0:'Normal Font' '{font_value}'"),
            &plist,
        ])
        .status()
        .context("failed to run PlistBuddy")?;

    if !status.success() {
        anyhow::bail!("PlistBuddy failed to update iTerm2 font");
    }

    // Tell iTerm2 to reload preferences.
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
    let url = format!(
        "https://github.com/ryanoasis/nerd-fonts/releases/latest/download/{font_name}.zip"
    );

    println!("  downloading {font_name} Nerd Font...");

    let resp =
        ureq::get(&url).call().with_context(|| format!("failed to download font from {url}"))?;

    let mut bytes = Vec::new();
    resp.into_reader()
        .read_to_end(&mut bytes)
        .context("failed to read font download")?;

    let font_dir = if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").context("HOME not set")?;
        PathBuf::from(home).join("Library/Fonts")
    } else {
        let home = std::env::var("HOME").context("HOME not set")?;
        PathBuf::from(home).join(".local/share/fonts")
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
        let _ = Command::new("fc-cache").arg("-f").status();
    }

    println!("  ✓ installed {installed} font files to {}", font_dir.display());

    // Resolve the real PostScript base name from the installed Regular font file.
    let regular_file = font_dir.join(format!("{filename_family}-Regular.ttf"));
    let family = postscript_base_name(&regular_file)
        .unwrap_or_else(|| filename_family.to_string());
    Ok(family)
}
