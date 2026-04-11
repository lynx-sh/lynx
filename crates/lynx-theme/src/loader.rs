use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lynx_core::error::{LynxError, Result};
use tracing::warn;

use crate::schema::{SegmentColor, Theme, KNOWN_SEGMENTS};

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

const BUILTIN_THEMES: &[(&str, &str)] = &[builtin!("default"), builtin!("minimal")];

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

/// Resolve `$varname` color strings in all segment configs against the `[colors]`
/// palette table. Runs once at load time — segments see plain hex/named values.
///
/// Rules:
/// - Only strings starting with `$` are resolved.
/// - If the palette key is not found the value is left unchanged (may be a valid
///   named color or hex — the renderer will handle it).
/// - The `[colors]` table itself is never mutated.
fn resolve_palette(theme: &mut Theme) {
    if theme.colors.is_empty() {
        return;
    }
    let palette = theme.colors.clone();

    for config in theme.segment.values_mut() {
        resolve_segment_color(&mut config.color, &palette);
        if let Some(ref mut si) = config.staged {
            resolve_opt_string(&mut si.color, &palette);
        }
        if let Some(ref mut si) = config.modified {
            resolve_opt_string(&mut si.color, &palette);
        }
        if let Some(ref mut si) = config.untracked {
            resolve_opt_string(&mut si.color, &palette);
        }
    }
}

fn resolve_segment_color(color: &mut Option<SegmentColor>, palette: &HashMap<String, String>) {
    if let Some(ref mut c) = color {
        resolve_opt_string(&mut c.fg, palette);
        resolve_opt_string(&mut c.bg, palette);
    }
}

/// If `s` starts with `$`, look up the remainder in `palette` and replace.
fn resolve_opt_string(s: &mut Option<String>, palette: &HashMap<String, String>) {
    if let Some(ref mut val) = s {
        if let Some(key) = val.strip_prefix('$') {
            if let Some(resolved) = palette.get(key) {
                *val = resolved.clone();
            }
        }
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
        if !KNOWN_SEGMENTS.contains(&seg.as_str()) {
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
        // dir segment has no config — falls back to SegmentConfig::default()
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
        let dir_fg = theme.segment["dir"].color.as_ref().unwrap().fg.as_deref().unwrap();
        assert_eq!(dir_fg, "#7aa2f7", "palette var '$accent' should resolve to '#7aa2f7'");
        let git_fg = theme.segment["git_branch"].color.as_ref().unwrap().fg.as_deref().unwrap();
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
        let fg = theme.segment["dir"].color.as_ref().unwrap().fg.as_deref().unwrap();
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
        let fg = theme.segment["dir"].color.as_ref().unwrap().fg.as_deref().unwrap();
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
}
