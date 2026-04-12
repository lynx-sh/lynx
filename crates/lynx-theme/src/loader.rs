use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lynx_core::error::{LynxError, Result};
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::schema::{Theme, KNOWN_SEGMENTS};

/// Filename for the lockfile that records sha256 checksums of built-in themes
/// as they were last written to the user theme dir. Used by resync to detect
/// stock (unmodified) files that are safe to overwrite on upgrade.
const CHECKSUM_FILE: &str = ".builtin-checksums.toml";

/// Compute the SHA-256 hex digest of `content`.
fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

/// Write (or overwrite) the checksum lockfile in `themes_dir` recording the
/// current embedded content of every built-in theme. Call this after copying
/// or writing built-in themes to the user directory (install + resync).
pub fn write_builtin_checksums(themes_dir: &Path) {
    let mut lines = String::from("# Lynx built-in theme checksums — do not edit\n");
    for (name, content) in BUILTIN_THEMES {
        lines.push_str(&format!("{name} = \"{}\"\n", sha256_hex(content.as_bytes())));
    }
    let path = themes_dir.join(CHECKSUM_FILE);
    if let Err(e) = std::fs::write(&path, &lines) {
        warn!("failed to write builtin checksums to {}: {e}", path.display());
    }
}

/// Read stored checksums from the lockfile. Returns an empty map if the file
/// is absent or unparseable (first install before checksums existed).
fn read_stored_checksums(themes_dir: &Path) -> HashMap<String, String> {
    let path = themes_dir.join(CHECKSUM_FILE);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let name = key.trim().to_string();
            let checksum = val.trim().trim_matches('"').to_string();
            map.insert(name, checksum);
        }
    }
    map
}

/// Resync built-in themes in `themes_dir` after a binary upgrade.
///
/// For each built-in theme:
/// - If the user's file doesn't exist: write it (fresh install path).
/// - If the user's file hash matches the stored checksum: the file is stock
///   (unmodified) — overwrite with the current embedded content.
/// - If the hash differs from the stored checksum: user has customized the
///   file — leave it untouched.
///
/// After processing, rewrites the checksum lockfile to reflect the current
/// built-in content so the next upgrade can detect stock files correctly.
///
/// Returns the number of files updated.
pub fn resync_builtin_themes(themes_dir: &Path) -> usize {
    let stored = read_stored_checksums(themes_dir);
    let mut updated = 0usize;

    for (name, content) in BUILTIN_THEMES {
        let user_path = themes_dir.join(format!("{name}.toml"));
        let new_hash = sha256_hex(content.as_bytes());

        let should_write = if user_path.exists() {
            match std::fs::read(&user_path) {
                Ok(bytes) => {
                    let user_hash = sha256_hex(&bytes);
                    // Already up to date — skip
                    if user_hash == new_hash {
                        continue;
                    }
                    // Stock check: user file hash matches what we last wrote
                    let stored_hash = stored.get(*name).map(String::as_str).unwrap_or("");
                    user_hash == stored_hash
                }
                Err(e) => {
                    warn!("cannot read {}: {e}", user_path.display());
                    false
                }
            }
        } else {
            true
        };

        if should_write {
            if let Err(e) = std::fs::write(&user_path, content.as_bytes()) {
                warn!("failed to resync theme {name}: {e}");
            } else {
                updated += 1;
                tracing::info!("resynced built-in theme: {name}");
            }
        }
    }

    // Refresh checksum lockfile to current built-in content.
    write_builtin_checksums(themes_dir);
    updated
}

/// Built-in themes bundled via `include_str!` so they ship with the binary
/// but remain editable in the `themes/` source directory.
macro_rules! builtin {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../themes/",
                $name,
                ".toml"
            )),
        )
    };
}

const BUILTIN_THEMES: &[(&str, &str)] = &[builtin!("default"), builtin!("minimal"), builtin!("rkj-repo"), builtin!("tokyo-night")];

/// Resolve the user theme directory: `~/.config/lynx/themes/`.
pub fn user_theme_dir() -> PathBuf {
    lynx_core::paths::themes_dir()
}

/// Load a theme by name. Checks user theme dir first, then built-ins.
pub fn load(name: &str) -> Result<Theme> {
    // 1. User theme dir
    let user_path = user_theme_dir().join(format!("{name}.toml"));
    if user_path.exists() {
        return load_from_path(&user_path);
    }

    // 2. Built-in themes
    for (builtin_name, content) in BUILTIN_THEMES {
        if *builtin_name == name {
            return parse_and_validate(content, name);
        }
    }

    Err(LynxError::Theme(format!("theme '{name}' not found")))
}

/// Load a theme from an explicit file path.
pub fn load_from_path(path: &Path) -> Result<Theme> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let content = std::fs::read_to_string(path).map_err(LynxError::IoRaw)?;
    parse_and_validate(&content, name)
}

/// Parse TOML content into a `Theme`, warning on unknown segments.
/// Palette variable references (`$varname`) in segment color fields are resolved
/// against the `[colors]` table before the theme is returned.
pub fn parse_and_validate(content: &str, name: &str) -> Result<Theme> {
    let mut theme: Theme = toml::from_str(content)
        .map_err(|e| LynxError::Theme(format!("parse error in theme '{name}': {e}")))?;

    resolve_palette(&mut theme);
    validate_segment_names(&theme, name);
    Ok(theme)
}

/// Resolve `$varname` strings in all segment config values against the `[colors]`
/// palette table. Runs once at load time — segments see plain hex/named values.
///
/// Walks the entire TOML value tree recursively so all string fields (color fg/bg,
/// status icon colors, any future fields) are resolved without enumerating field names.
///
/// Rules:
/// - Only strings starting with `$` are resolved.
/// - Unknown palette keys are left unchanged (may be a valid named color or hex).
/// - The `[colors]` table itself is never mutated.
fn resolve_palette(theme: &mut Theme) {
    if theme.colors.is_empty() {
        return;
    }
    let palette = theme.colors.clone();
    for config in theme.segment.values_mut() {
        resolve_value(&mut *config, &palette);
    }
    resolve_ls_colors_palette(&mut theme.ls_colors, &palette);
}

/// Resolve `$varname` palette references inside the typed `LsColors` struct.
fn resolve_ls_colors_palette(
    lsc: &mut crate::schema::LsColors,
    palette: &HashMap<String, String>,
) {
    for entry in [
        &mut lsc.dir,
        &mut lsc.symlink,
        &mut lsc.executable,
        &mut lsc.archive,
        &mut lsc.image,
        &mut lsc.audio,
        &mut lsc.broken,
        &mut lsc.other_writable,
    ]
    .into_iter()
    .flatten()
    {
        resolve_color_ref(&mut entry.fg, palette);
        resolve_color_ref(&mut entry.bg, palette);
    }
}

fn resolve_color_ref(field: &mut Option<String>, palette: &HashMap<String, String>) {
    if let Some(s) = field {
        if let Some(key) = s.strip_prefix('$') {
            if let Some(resolved) = palette.get(key) {
                *s = resolved.clone();
            }
        }
    }
}

/// Recursively resolve `$varname` strings in a `toml::Value`.
fn resolve_value(value: &mut toml::Value, palette: &HashMap<String, String>) {
    match value {
        toml::Value::String(s) => {
            if let Some(key) = s.strip_prefix('$') {
                if let Some(resolved) = palette.get(key) {
                    *s = resolved.clone();
                }
            }
        }
        toml::Value::Table(map) => {
            for (_, v) in map.iter_mut() {
                resolve_value(v, palette);
            }
        }
        toml::Value::Array(arr) => {
            for v in arr.iter_mut() {
                resolve_value(v, palette);
            }
        }
        _ => {}
    }
}

fn validate_segment_names(theme: &Theme, name: &str) {
    let all_segments = theme
        .segments
        .left
        .order
        .iter()
        .chain(theme.segments.right.order.iter());

    for seg in all_segments {
        if !KNOWN_SEGMENTS.contains(&seg.as_str()) && !seg.starts_with("custom_") {
            warn!("theme '{name}': unknown segment '{seg}' in order array — ignoring");
        }
    }
}

/// Return the raw TOML content for a built-in theme by name.
pub fn builtin_content(name: &str) -> Option<&'static str> {
    BUILTIN_THEMES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, content)| *content)
}

/// List all available theme names (built-in + user).
pub fn list() -> Vec<String> {
    let mut names: Vec<String> = BUILTIN_THEMES.iter().map(|(n, _)| n.to_string()).collect();

    let user_dir = user_theme_dir();
    if let Ok(entries) = std::fs::read_dir(&user_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !names.contains(&stem.to_string()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
    }

    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_default_builtin() {
        let theme = load("default").expect("default theme must load");
        assert_eq!(theme.meta.name, "default");
        assert!(!theme.segments.left.order.is_empty());
        assert!(!theme.segments.right.order.is_empty());
    }


    #[test]
    fn unknown_segment_warns_not_errors() {
        let toml = r#"
[meta]
name = "test"

[segments.left]
order = ["dir", "unknown_segment_xyz"]

[segments.right]
order = []
"#;
        // Should succeed — unknown segment produces a warning, not an error.
        let theme = parse_and_validate(toml, "test").expect("should not error on unknown segment");
        assert!(theme
            .segments
            .left
            .order
            .contains(&"unknown_segment_xyz".to_string()));
    }

    #[test]
    fn missing_color_field_falls_back() {
        let toml = r#"
[meta]
name = "no-colors"

[segments.left]
order = ["dir"]

[segments.right]
order = []
"#;
        let theme = parse_and_validate(toml, "no-colors").unwrap();
        // No colors table — defaults to empty HashMap
        assert!(theme.colors.is_empty());
        // dir segment has no config — evaluator uses empty toml::Value table
        assert!(!theme.segment.contains_key("dir"));
    }

    #[test]
    fn meta_fields_all_parsed() {
        let theme = load("default").unwrap();
        assert!(!theme.meta.name.is_empty());
        assert!(!theme.meta.description.is_empty());
        assert!(!theme.meta.author.is_empty());
    }

    #[test]
    fn nonexistent_theme_errors() {
        assert!(load("does_not_exist_xyz").is_err());
    }

    #[test]
    fn list_includes_builtins() {
        let names = list();
        assert!(names.contains(&"default".to_string()));
        assert!(names.contains(&"minimal".to_string()));
    }

    #[test]
    fn palette_vars_resolved_in_segment_colors() {
        let toml = r###"
[meta]
name = "palette-test"

[colors]
accent = "#7aa2f7"
danger = "#f7768e"

[segments.left]
order = ["dir", "git_branch"]

[segments.right]
order = []

[segment.dir]
color = { fg = "$accent" }

[segment.git_branch]
color = { fg = "$danger", bold = true }
"###;
        let theme = parse_and_validate(toml, "palette-test").unwrap();
        let dir_fg = theme.segment["dir"]
            .get("color").and_then(|c| c.get("fg")).and_then(|v| v.as_str()).unwrap();
        assert_eq!(dir_fg, "#7aa2f7", "palette var '$accent' should resolve to '#7aa2f7'");
        let git_fg = theme.segment["git_branch"]
            .get("color").and_then(|c| c.get("fg")).and_then(|v| v.as_str()).unwrap();
        assert_eq!(git_fg, "#f7768e", "palette var '$danger' should resolve to '#f7768e'");
    }

    #[test]
    fn unknown_palette_var_left_as_is() {
        let toml = r###"
[meta]
name = "unknown-var"

[colors]
accent = "#7aa2f7"

[segments.left]
order = ["dir"]

[segments.right]
order = []

[segment.dir]
color = { fg = "$nonexistent" }
"###;
        let theme = parse_and_validate(toml, "unknown-var").unwrap();
        // $nonexistent not in [colors] — left unchanged (may be a valid color name)
        let fg = theme.segment["dir"]
            .get("color").and_then(|c| c.get("fg")).and_then(|v| v.as_str()).unwrap();
        assert_eq!(fg, "$nonexistent");
    }

    #[test]
    fn non_var_colors_unchanged() {
        let toml = r###"
[meta]
name = "literal-colors"

[colors]
accent = "#7aa2f7"

[segments.left]
order = ["dir"]

[segments.right]
order = []

[segment.dir]
color = { fg = "blue" }
"###;
        let theme = parse_and_validate(toml, "literal-colors").unwrap();
        let fg = theme.segment["dir"]
            .get("color").and_then(|c| c.get("fg")).and_then(|v| v.as_str()).unwrap();
        assert_eq!(fg, "blue", "literal color names must not be modified by palette resolver");
    }

    #[test]
    fn load_from_path_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom.toml");
        std::fs::write(
            &path,
            r#"
[meta]
name = "custom"
description = "test"

[segments.left]
order = ["dir"]

[segments.right]
order = []
"#,
        )
        .unwrap();
        let theme = load_from_path(&path).unwrap();
        assert_eq!(theme.meta.name, "custom");
    }

    // ── Resync tests ──────────────────────────────────────────────────────

    #[test]
    fn resync_writes_missing_theme_files() {
        let dir = tempfile::tempdir().unwrap();
        let n = resync_builtin_themes(dir.path());
        // All built-ins should be written since none exist yet.
        assert_eq!(n, BUILTIN_THEMES.len(), "all built-in themes should be written");
        for (name, _) in BUILTIN_THEMES {
            assert!(dir.path().join(format!("{name}.toml")).exists());
        }
        // Checksum lockfile must be present.
        assert!(dir.path().join(CHECKSUM_FILE).exists());
    }

    /// Seed a themes dir with all built-in files (as "old" content) and matching checksums.
    /// Used to isolate per-theme behaviour in resync tests.
    fn seed_all_stock(dir: &std::path::Path, old_content: &[u8]) -> String {
        let mut lockfile = String::new();
        for (name, _) in BUILTIN_THEMES {
            std::fs::write(dir.join(format!("{name}.toml")), old_content).unwrap();
            lockfile.push_str(&format!("{name} = \"{}\"\n", sha256_hex(old_content)));
        }
        lockfile
    }

    #[test]
    fn resync_overwrites_stock_file() {
        let dir = tempfile::tempdir().unwrap();
        let (name, content) = BUILTIN_THEMES[0];
        let path = dir.path().join(format!("{name}.toml"));

        // Simulate previous version: all themes are the same "old" stock content.
        let old_content = b"# old built-in content";
        let lockfile_content = seed_all_stock(dir.path(), old_content);
        std::fs::write(dir.path().join(CHECKSUM_FILE), &lockfile_content).unwrap();

        let n = resync_builtin_themes(dir.path());
        // Every file is stock → all should be overwritten
        assert_eq!(n, BUILTIN_THEMES.len(), "all stock files should be overwritten");
        let actual = std::fs::read_to_string(&path).unwrap();
        assert_eq!(actual, *content, "first theme should contain current built-in content");
    }

    #[test]
    fn resync_preserves_user_customized_file() {
        let dir = tempfile::tempdir().unwrap();
        let (name, _) = BUILTIN_THEMES[0];
        let path = dir.path().join(format!("{name}.toml"));

        // All themes start as stock old content.
        let old_content = b"# old built-in content";
        let lockfile_content = seed_all_stock(dir.path(), old_content);
        std::fs::write(dir.path().join(CHECKSUM_FILE), &lockfile_content).unwrap();

        // User has modified the first theme — its hash no longer matches the stored checksum.
        let user_content = b"# user customized theme";
        std::fs::write(&path, user_content).unwrap();

        let n = resync_builtin_themes(dir.path());
        // Only the remaining (non-customized) themes should be resynced.
        assert_eq!(n, BUILTIN_THEMES.len() - 1, "customized file must not be overwritten");
        let after = std::fs::read(&path).unwrap();
        assert_eq!(after, user_content, "user content must be preserved");
    }

    #[test]
    fn resync_skips_already_current_file() {
        let dir = tempfile::tempdir().unwrap();
        // Write all built-ins as if freshly synced.
        let n1 = resync_builtin_themes(dir.path());
        assert_eq!(n1, BUILTIN_THEMES.len());
        // Second resync: files already match current built-in — nothing to do.
        let n2 = resync_builtin_themes(dir.path());
        assert_eq!(n2, 0, "no files should be resynced when already up to date");
    }

    #[test]
    fn write_and_read_checksums_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        write_builtin_checksums(dir.path());
        let stored = read_stored_checksums(dir.path());
        for (name, content) in BUILTIN_THEMES {
            let expected = sha256_hex(content.as_bytes());
            assert_eq!(stored.get(*name), Some(&expected));
        }
    }
}
