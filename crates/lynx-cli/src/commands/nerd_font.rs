use std::io::Read as _;
use std::path::PathBuf;

use anyhow::{Context as _, Result};

/// Check if a theme uses Powerline / Nerd Font glyphs in its separators.
pub fn theme_needs_nerd_font(theme: &lynx_theme::Theme) -> bool {
    let check_glyph = |s: &Option<String>| -> bool {
        s.as_ref().map(|c| {
            c.chars().any(|ch| {
                // Powerline glyphs: U+E0A0–U+E0D4
                // Nerd Font extras: U+E200–U+E2FF, U+F000–U+F2FF
                let cp = ch as u32;
                (0xE0A0..=0xE0D4).contains(&cp)
                    || (0xE200..=0xE2FF).contains(&cp)
                    || (0xF000..=0xF2FF).contains(&cp)
            })
        }).unwrap_or(false)
    };
    check_glyph(&theme.separators.left.char)
        || check_glyph(&theme.separators.right.char)
        || check_glyph(&theme.separators.left_edge.char)
        || check_glyph(&theme.separators.right_edge.char)
}

/// Check if any Nerd Font is installed on the system.
pub fn nerd_font_installed() -> bool {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let font_dirs: Vec<PathBuf> = {
        let mut dirs = Vec::new();
        // macOS
        if let Some(ref h) = home {
            dirs.push(h.join("Library/Fonts"));
        }
        dirs.push("/Library/Fonts".into());
        dirs.push("/System/Library/Fonts".into());
        // Linux
        if let Some(ref h) = home {
            dirs.push(h.join(".local/share/fonts"));
            dirs.push(h.join(".fonts"));
        }
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        dirs
    };

    for dir in &font_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.contains("nerd") || name.contains("powerline") {
                    return true;
                }
                // Check subdirectories (fonts are often in family folders).
                if entry.path().is_dir() {
                    if let Ok(sub) = std::fs::read_dir(entry.path()) {
                        for sub_entry in sub.flatten() {
                            let sub_name = sub_entry.file_name().to_string_lossy().to_lowercase();
                            if sub_name.contains("nerd") || sub_name.contains("powerline") {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Download and install a Nerd Font (FiraCode by default).
pub fn install_nerd_font() -> Result<()> {
    let font_name = "FiraCode";
    let url = format!(
        "https://github.com/ryanoasis/nerd-fonts/releases/latest/download/{font_name}.zip"
    );

    eprintln!("  downloading {font_name} Nerd Font...");

    let resp = ureq::get(&url).call()
        .with_context(|| format!("failed to download font from {url}"))?;

    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes)
        .context("failed to read font download")?;

    // Determine install directory.
    let font_dir = if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").context("HOME not set")?;
        PathBuf::from(home).join("Library/Fonts")
    } else {
        let home = std::env::var("HOME").context("HOME not set")?;
        PathBuf::from(home).join(".local/share/fonts")
    };
    std::fs::create_dir_all(&font_dir)?;

    // Extract .ttf and .otf files from the zip.
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .context("failed to open font zip archive")?;

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

    // Refresh font cache on Linux.
    if cfg!(target_os = "linux") {
        let _ = std::process::Command::new("fc-cache").arg("-f").status();
    }

    eprintln!("  ✓ installed {installed} font files to {}", font_dir.display());
    eprintln!("  → Set your terminal font to \"{font_name} Nerd Font\" in terminal preferences.");
    Ok(())
}
