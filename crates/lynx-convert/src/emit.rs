use crate::omp::ConvertedTheme;
use crate::omz::{OmzTheme, Tier};

/// Convert a `ConvertedTheme` (from OMP parser) to a Lynx theme TOML string.
/// Per D-038: extracts unique colors into a [colors] palette with semantic names,
/// and references them via $varname in segment configs.
pub fn omp_to_lynx_toml(theme: &ConvertedTheme, name: &str) -> String {
    let mut out = String::new();

    // Header.
    out.push_str(&format!(
        "# Lynx theme converted from Oh-My-Posh: {name}\n# Auto-generated — review and adjust colors\n"
    ));
    for note in &theme.notes {
        out.push_str(&format!("# NOTE: {note}\n"));
    }
    out.push('\n');

    // Meta.
    out.push_str("[meta]\n");
    out.push_str(&format!("name = \"{name}\"\n"));
    out.push_str(&format!("description = \"Converted from Oh-My-Posh {name} theme\"\n"));
    out.push_str("author = \"auto-converted\"\n\n");

    // Palette — build from all unique colors, generate semantic names.
    let mut palette: Vec<(String, String)> = Vec::new();
    let mut color_to_var: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    let all_segments: Vec<&crate::omp::ConvertedSegment> = theme
        .top.iter()
        .chain(theme.top_right.iter())
        .chain(theme.left.iter())
        .collect();

    for seg in &all_segments {
        for hex in [&seg.fg, &seg.bg].into_iter().flatten() {
            if hex.starts_with('#') && !color_to_var.contains_key(hex) {
                let var_name = format!("{}_{}", seg.name, if Some(hex) == seg.fg.as_ref() { "fg" } else { "bg" });
                // Deduplicate: if this var name exists, append a counter.
                let unique_name = if palette.iter().any(|(k, _)| k == &var_name) {
                    format!("{}_{}", var_name, palette.len())
                } else {
                    var_name
                };
                color_to_var.insert(hex.clone(), unique_name.clone());
                palette.push((unique_name, hex.clone()));
            }
        }
    }

    if !palette.is_empty() {
        out.push_str("[colors]\n");
        for (var_name, hex) in &palette {
            out.push_str(&format!("{var_name} = \"{hex}\"\n"));
        }
        out.push('\n');
    }

    // Layout.
    if theme.two_line {
        // Two-line: top segments on info line, left on input line.
        if !theme.top.is_empty() {
            out.push_str("[segments.top]\n");
            out.push_str(&format!("order = [{}]\n", format_order(&theme.top)));
        }
        if !theme.top_right.is_empty() {
            out.push_str("\n[segments.top_right]\n");
            out.push_str(&format!("order = [{}]\n", format_order(&theme.top_right)));
        }
        if !theme.left.is_empty() {
            out.push_str("\n[segments.left]\n");
            out.push_str(&format!("order = [{}]\n", format_order(&theme.left)));
        }
        out.push_str("\n[segments.right]\norder = []\n");
    } else {
        // Single line.
        if !theme.left.is_empty() {
            out.push_str("[segments.left]\n");
            out.push_str(&format!("order = [{}]\n", format_order(&theme.left)));
        }
        if !theme.top_right.is_empty() {
            out.push_str("\n[segments.right]\n");
            out.push_str(&format!("order = [{}]\n", format_order(&theme.top_right)));
        } else {
            out.push_str("\n[segments.right]\norder = []\n");
        }
    }
    out.push('\n');

    // Filler.
    if let Some(ref filler) = theme.filler {
        out.push_str("[segments.filler]\n");
        out.push_str(&format!("char = \"{filler}\"\n\n"));
    }

    // Separators — use adaptive mode if any segment has bg colors.
    let has_bg = all_segments.iter().any(|s| s.bg.is_some());
    if has_bg {
        out.push_str("[separators]\nmode = \"adaptive\"\n\n");
        out.push_str("[separators.left]\nchar = \"\\ue0b0\"\n\n");
    }

    // Transient prompt.
    if let Some(ref t) = theme.transient {
        out.push_str("[transient]\n");
        out.push_str(&format!("template = \"{}\"\n", escape_toml(&t.template)));
        if let Some(ref fg) = t.fg {
            let var = color_to_var.get(fg).map(|v| format!("${v}")).unwrap_or_else(|| fg.clone());
            out.push_str(&format!("fg = \"{var}\"\n"));
        }
        out.push('\n');
    }

    // Per-segment config.
    for seg in &all_segments {
        let section = format!("[segment.{}]", seg.name);
        let mut fields = Vec::new();

        // Color — use palette vars per D-015.
        let fg_ref = seg.fg.as_ref().and_then(|h| color_to_var.get(h)).map(|v| format!("${v}"));
        let bg_ref = seg.bg.as_ref().and_then(|h| color_to_var.get(h)).map(|v| format!("${v}"));
        let fg_val = fg_ref.or_else(|| seg.fg.clone());
        let bg_val = bg_ref.or_else(|| seg.bg.clone());

        if fg_val.is_some() || bg_val.is_some() {
            let mut color_parts = Vec::new();
            if let Some(ref fg) = fg_val {
                color_parts.push(format!("fg = \"{fg}\""));
            }
            if let Some(ref bg) = bg_val {
                color_parts.push(format!("bg = \"{bg}\""));
            }
            fields.push(format!("color = {{ {} }}", color_parts.join(", ")));
        }

        // Per-segment separators.
        if let Some(ref lc) = seg.leading_char {
            fields.push(format!("leading_char = \"{}\"", escape_toml(lc)));
        }
        if let Some(ref tc) = seg.trailing_char {
            fields.push(format!("trailing_char = \"{}\"", escape_toml(tc)));
        }

        // Text content.
        if let Some(ref content) = seg.content {
            if !content.is_empty() {
                fields.push(format!("content = \"{}\"", escape_toml(content)));
            }
        }

        // Icon.
        if let Some(ref icon) = seg.icon {
            fields.push(format!("icon = \"{}\"", escape_toml(icon)));
        }

        if !fields.is_empty() {
            out.push_str(&section);
            out.push('\n');
            for f in &fields {
                out.push_str(f);
                out.push('\n');
            }
            out.push('\n');
        }
    }

    out
}

fn format_order(segs: &[crate::omp::ConvertedSegment]) -> String {
    segs.iter()
        .map(|s| format!("\"{}\"", s.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn escape_toml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Convert an OmzTheme IR to a Lynx theme TOML string.
pub fn to_lynx_toml(theme: &OmzTheme, name: &str) -> String {
    let mut out = String::new();

    // Header comments.
    out.push_str(&format!(
        "# Lynx theme converted from OMZ: {name}\n# Auto-generated — review and adjust colors\n"
    ));

    if theme.tier == Tier::Agnoster {
        out.push_str("# NOTE: agnoster-style theme — segment order approximated, colors manually tuned\n");
    }

    for note in &theme.notes {
        out.push_str(&format!("# NOTE: {note}\n"));
    }
    out.push('\n');

    // Meta section.
    out.push_str("[meta]\n");
    out.push_str(&format!("name = \"{name}\"\n"));
    out.push_str(&format!("description = \"Converted from OMZ {name} theme\"\n"));
    out.push_str("author = \"auto-converted\"\n\n");

    // Layout section.
    out.push_str("[layout]\n");
    if !theme.left.is_empty() {
        out.push_str(&format!(
            "left = [{}]\n",
            theme.left.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ")
        ));
    }
    if !theme.right.is_empty() {
        out.push_str(&format!(
            "right = [{}]\n",
            theme.right.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ")
        ));
    }
    if theme.two_line {
        out.push_str("# Original theme was two-line — configure top/top_right for two-line layout\n");
    }
    out.push('\n');

    // Segment colors.
    for (seg_name, color) in &theme.colors {
        out.push_str(&format!("[segment.{seg_name}]\n"));
        let mut color_parts = Vec::new();
        if let Some(ref fg) = color.fg {
            color_parts.push(format!("fg = \"{fg}\""));
        }
        if color.bold {
            color_parts.push("bold = true".to_string());
        }
        if !color_parts.is_empty() {
            out.push_str(&format!("color = {{ {} }}\n", color_parts.join(", ")));
        }
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omz;

    #[test]
    fn emitted_toml_roundtrips() {
        let content = r#"PROMPT='%{$fg[cyan]%}%c%{$reset_color%} $(git_prompt_info)❯ '"#;
        let theme = omz::parse(content);
        let toml_str = to_lynx_toml(&theme, "test");
        // Should be valid TOML.
        let parsed: Result<toml::Value, _> = toml::from_str(&toml_str);
        assert!(parsed.is_ok(), "invalid TOML: {}\n{}", parsed.unwrap_err(), toml_str);
    }

    #[test]
    fn agnoster_has_note_comment() {
        let content = "prompt_segment() { }\nbuild_prompt() { prompt_dir; }";
        let theme = omz::parse(content);
        let toml_str = to_lynx_toml(&theme, "agnoster");
        assert!(toml_str.contains("agnoster-style"));
    }

    #[test]
    fn meta_section_present() {
        let theme = OmzTheme::default();
        let toml_str = to_lynx_toml(&theme, "myname");
        assert!(toml_str.contains("name = \"myname\""));
    }
}
