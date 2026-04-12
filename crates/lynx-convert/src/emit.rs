use crate::omz::{OmzTheme, Tier};

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
