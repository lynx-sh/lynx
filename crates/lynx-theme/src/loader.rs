use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lynx_core::error::{LynxError, Result};
use tracing::warn;

use crate::schema::{Theme, KNOWN_SEGMENTS};

/// Resolve the user theme directory: `~/.config/lynx/themes/`.
pub fn user_theme_dir() -> PathBuf {
    lynx_core::paths::themes_dir()
}

/// Load a theme by name from the themes directory.
pub fn load(name: &str) -> Result<Theme> {
    let path = user_theme_dir().join(format!("{name}.toml"));
    if path.exists() {
        return load_from_path(&path);
    }
    Err(LynxError::Theme(format!("theme '{name}' not found — run `lx install` to set up default themes")))
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

/// List all available theme names from the themes directory.
pub fn list() -> Vec<String> {
    let mut names = Vec::new();
    let user_dir = user_theme_dir();
    if let Ok(entries) = std::fs::read_dir(&user_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
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

    /// Helper: write a theme TOML to a temp dir and load via load_from_path.
    const DEFAULT_THEME_TOML: &str = include_str!("../../../themes/default.toml");

    #[test]
    fn load_default_from_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("default.toml"), DEFAULT_THEME_TOML).unwrap();
        std::env::set_var("LYNX_DIR", dir.path());
        std::fs::create_dir_all(dir.path().join("themes")).unwrap();
        std::fs::write(dir.path().join("themes/default.toml"), DEFAULT_THEME_TOML).unwrap();
        let theme = load_from_path(&dir.path().join("themes/default.toml")).unwrap();
        assert_eq!(theme.meta.name, "default");
        assert!(!theme.segments.left.order.is_empty());
        std::env::remove_var("LYNX_DIR");
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
        let theme = parse_and_validate(toml, "test").expect("should not error on unknown segment");
        assert!(theme.segments.left.order.contains(&"unknown_segment_xyz".to_string()));
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
        assert!(theme.colors.is_empty());
        assert!(!theme.segment.contains_key("dir"));
    }

    #[test]
    fn meta_fields_all_parsed() {
        let theme = parse_and_validate(DEFAULT_THEME_TOML, "default").unwrap();
        assert!(!theme.meta.name.is_empty());
        assert!(!theme.meta.description.is_empty());
        assert!(!theme.meta.author.is_empty());
    }

    #[test]
    fn nonexistent_theme_errors() {
        assert!(load("does_not_exist_xyz").is_err());
    }

    #[test]
    fn list_reads_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let themes_dir = dir.path().join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("alpha.toml"), "").unwrap();
        std::fs::write(themes_dir.join("beta.toml"), "").unwrap();
        std::env::set_var("LYNX_DIR", dir.path());
        let names = list();
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
        std::env::remove_var("LYNX_DIR");
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
        assert_eq!(dir_fg, "#7aa2f7");
        let git_fg = theme.segment["git_branch"]
            .get("color").and_then(|c| c.get("fg")).and_then(|v| v.as_str()).unwrap();
        assert_eq!(git_fg, "#f7768e");
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
        assert_eq!(fg, "blue");
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
}
