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

/// Find installed Nerd Font names on disk. Returns the font family name (e.g. "JetBrainsMonoNerdFont").
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

    let mut families = std::collections::HashSet::new();
    for dir in &font_dirs {
        scan_dir_for_nerd_fonts(dir, &mut families);
    }
    let mut result: Vec<String> = families.into_iter().collect();
    result.sort();
    result
}

fn scan_dir_for_nerd_fonts(dir: &PathBuf, families: &mut std::collections::HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() {
            scan_dir_for_nerd_fonts(&entry.path(), families);
        } else if name.to_lowercase().contains("nerd") {
            // Extract family name from filename: "JetBrainsMonoNerdFont-Regular.ttf" → "JetBrainsMonoNerdFont"
            if let Some(stem) = name.strip_suffix(".ttf").or_else(|| name.strip_suffix(".otf")) {
                if let Some(family) = stem.split('-').next() {
                    families.insert(family.to_string());
                }
            }
        }
    }
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
            lower.contains("nerd") || lower.contains("powerline")
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
    let family = "FiraCodeNerdFont";
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
    Ok(family.to_string())
}
