//! Oh-My-Posh JSON theme parser.
//!
//! Parses OMP v4 JSON themes into a `ConvertedTheme` IR that the emitter
//! converts to Lynx TOML. Does NOT replicate OMP's Go template runtime —
//! extracts static content, icons, colors, and layout.

use serde::Deserialize;
use std::collections::HashMap;

/// Intermediate representation of a converted OMP theme.
#[derive(Debug, Clone, Default)]
pub struct ConvertedTheme {
    /// Segments on the first line (left-aligned).
    pub top: Vec<ConvertedSegment>,
    /// Segments on the first line (right-aligned).
    pub top_right: Vec<ConvertedSegment>,
    /// Segments on the second line (left-aligned, the input line).
    pub left: Vec<ConvertedSegment>,
    /// Whether the theme uses a two-line layout.
    pub two_line: bool,
    /// Transient prompt template (if defined).
    pub transient: Option<TransientPrompt>,
    /// Unique colors extracted into a palette (semantic_name → hex).
    pub palette: HashMap<String, String>,
    /// Filler config (char between left and right on same line).
    pub filler: Option<String>,
    /// Notes about unsupported features or approximations.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ConvertedSegment {
    /// Lynx segment name (mapped from OMP type).
    pub name: String,
    /// Foreground color (hex).
    pub fg: Option<String>,
    /// Background color (hex).
    pub bg: Option<String>,
    /// Leading separator char (diamond leading_diamond).
    pub leading_char: Option<String>,
    /// Trailing separator char (diamond trailing_diamond or powerline symbol).
    pub trailing_char: Option<String>,
    /// Static text content extracted from template (for 'text' segments).
    pub content: Option<String>,
    /// Icon extracted from template.
    pub icon: Option<String>,
    /// The original OMP segment type (for notes).
    pub omp_type: String,
}

#[derive(Debug, Clone)]
pub struct TransientPrompt {
    pub template: String,
    pub fg: Option<String>,
    pub bg: Option<String>,
}

// ── OMP JSON schema (subset needed for parsing) ──────────────────────────────

#[derive(Deserialize)]
struct OmpTheme {
    #[serde(default)]
    blocks: Vec<OmpBlock>,
    #[serde(default)]
    transient_prompt: Option<OmpTransient>,
    #[serde(default)]
    palette: HashMap<String, String>,
    #[serde(default)]
    version: u32,
}

#[derive(Deserialize)]
struct OmpBlock {
    #[serde(default)]
    alignment: String,
    #[serde(default)]
    segments: Vec<OmpSegment>,
    #[serde(default)]
    newline: bool,
    #[serde(default)]
    filler: Option<String>,
    #[serde(rename = "type", default)]
    block_type: String,
}

#[derive(Deserialize)]
struct OmpSegment {
    #[serde(rename = "type")]
    seg_type: String,
    #[serde(default)]
    style: String,
    #[serde(default)]
    foreground: String,
    #[serde(default)]
    background: String,
    #[serde(default)]
    template: String,
    #[serde(default)]
    leading_diamond: String,
    #[serde(default)]
    trailing_diamond: String,
    #[serde(default)]
    powerline_symbol: String,
    #[serde(default)]
    properties: Option<serde_json::Value>,
    #[serde(default)]
    options: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct OmpTransient {
    #[serde(default)]
    template: String,
    #[serde(default)]
    foreground: String,
    #[serde(default)]
    background: String,
}

/// Parse an OMP JSON theme string into a `ConvertedTheme`.
pub fn parse(json: &str) -> Result<ConvertedTheme, String> {
    let omp: OmpTheme = serde_json::from_str(json)
        .map_err(|e| format!("OMP JSON parse error: {e}"))?;

    if omp.version > 0 && omp.version < 2 {
        return Err("OMP v1 themes are not supported — upgrade to v2+ first".to_string());
    }

    let mut theme = ConvertedTheme::default();
    let mut palette_counter = 0u32;

    // Merge OMP palette into our palette.
    for (k, v) in &omp.palette {
        theme.palette.insert(k.clone(), v.clone());
    }

    // Process blocks in order.
    let mut is_first_block = true;
    for block in &omp.blocks {
        if block.block_type == "rprompt" {
            // Right prompt — map to top_right (OMP right prompts appear on same line).
            for seg in &block.segments {
                theme.top_right.push(convert_segment(seg, &mut theme.palette, &mut palette_counter));
            }
            continue;
        }

        // Check for filler.
        if let Some(ref filler) = block.filler {
            if !filler.is_empty() {
                theme.filler = Some(filler.clone());
            }
        }

        let is_newline_block = block.newline;

        if is_first_block && !is_newline_block {
            // First block, no newline — these go on the top/left line.
            match block.alignment.as_str() {
                "right" => {
                    for seg in &block.segments {
                        theme.top_right.push(convert_segment(seg, &mut theme.palette, &mut palette_counter));
                    }
                }
                _ => {
                    for seg in &block.segments {
                        theme.top.push(convert_segment(seg, &mut theme.palette, &mut palette_counter));
                    }
                }
            }
        } else if is_newline_block {
            // Newline block — this is the input line (second line).
            theme.two_line = true;
            for seg in &block.segments {
                theme.left.push(convert_segment(seg, &mut theme.palette, &mut palette_counter));
            }
        } else {
            // Subsequent blocks without newline — right-aligned on first line.
            match block.alignment.as_str() {
                "right" => {
                    for seg in &block.segments {
                        theme.top_right.push(convert_segment(seg, &mut theme.palette, &mut palette_counter));
                    }
                }
                _ => {
                    // Additional left block — append to top.
                    for seg in &block.segments {
                        theme.top.push(convert_segment(seg, &mut theme.palette, &mut palette_counter));
                    }
                }
            }
        }

        is_first_block = false;
    }

    // If no two-line layout detected, move top segments to left.
    if !theme.two_line && !theme.top.is_empty() {
        theme.left = std::mem::take(&mut theme.top);
    }

    // Transient prompt.
    if let Some(ref t) = omp.transient_prompt {
        if !t.template.is_empty() {
            theme.transient = Some(TransientPrompt {
                template: extract_static_text(&t.template),
                fg: non_empty(&t.foreground),
                bg: non_empty(&t.background),
            });
        }
    }

    Ok(theme)
}

#[allow(unused_variables)]
fn convert_segment(
    seg: &OmpSegment,
    palette: &mut HashMap<String, String>,
    counter: &mut u32,
) -> ConvertedSegment {
    let name = map_segment_type(&seg.seg_type);

    // Extract icon from template.
    let icon = extract_icon(&seg.template);
    let content = if name == "text" {
        Some(extract_static_text(&seg.template))
    } else {
        None
    };

    // Extract separator chars based on style.
    let (leading_char, trailing_char) = match seg.style.as_str() {
        "diamond" => (
            non_empty(&seg.leading_diamond),
            non_empty(&seg.trailing_diamond),
        ),
        "powerline" => (
            None,
            non_empty(&seg.powerline_symbol),
        ),
        _ => (None, None),
    };

    // Register colors in palette with semantic names.
    let fg = resolve_omp_color(&seg.foreground, palette, counter);
    let bg = resolve_omp_color(&seg.background, palette, counter);

    ConvertedSegment {
        name,
        fg,
        bg,
        leading_char,
        trailing_char,
        content,
        icon,
        omp_type: seg.seg_type.clone(),
    }
}

/// Resolve an OMP color reference. OMP supports:
/// - Hex: "#RRGGBB"
/// - Palette: "p:name"
/// - Special: "transparent", "parentBackground", "parentForeground"
#[allow(unused_variables)]
fn resolve_omp_color(
    color: &str,
    palette: &mut HashMap<String, String>,
    counter: &mut u32,
) -> Option<String> {
    let color = color.trim();
    if color.is_empty() {
        return None;
    }

    // Palette reference: p:name
    if let Some(name) = color.strip_prefix("p:") {
        if let Some(hex) = palette.get(name) {
            return Some(hex.clone());
        }
        return Some(color.to_string()); // Unresolved palette ref
    }

    // Hex color
    if color.starts_with('#') {
        return Some(color.to_string());
    }

    // Special OMP keywords
    match color {
        "transparent" | "parentBackground" | "parentForeground" |
        "background" | "foreground" => None, // Can't map these meaningfully
        _ => {
            // Named color — pass through (our color system handles names)
            Some(color.to_string())
        }
    }
}

/// Map OMP segment type to Lynx segment name.
fn map_segment_type(omp_type: &str) -> String {
    match omp_type {
        // Core segments
        "path" => "dir".to_string(),
        "git" => "git_branch".to_string(),
        "executiontime" => "cmd_duration".to_string(),
        "exit" | "status" => "exit_code".to_string(),
        "time" => "time".to_string(),
        "session" => "username".to_string(),
        "os" => "os".to_string(),
        "shell" => "shell".to_string(),
        "text" => "text".to_string(),
        "root" => "username".to_string(),
        "battery" => "battery".to_string(),

        // Language segments → lang_version (our unified detector)
        "node" => "lang_version".to_string(),
        "python" => "lang_version".to_string(),
        "go" => "lang_version".to_string(),
        "rust" => "lang_version".to_string(),
        "java" => "lang_version".to_string(),
        "dotnet" => "lang_version".to_string(),
        "ruby" => "lang_version".to_string(),
        "php" => "lang_version".to_string(),
        "dart" => "lang_version".to_string(),
        "swift" => "lang_version".to_string(),
        "kotlin" => "lang_version".to_string(),
        "elixir" => "lang_version".to_string(),
        "lua" => "lang_version".to_string(),
        "zig" => "lang_version".to_string(),

        // Cloud/DevOps segments
        "kubectl" => "kubectl_context".to_string(),
        "aws" => "aws_profile".to_string(),
        "docker" => "docker".to_string(),
        "terraform" => "terraform".to_string(),
        "gcp" => "gcp".to_string(),

        // Segments we don't have → text fallback
        _ => "text".to_string(),
    }
}

/// Extract the first Unicode icon character from an OMP template string.
/// OMP templates use Nerd Font chars like \uf120, \ue718, etc.
fn extract_icon(template: &str) -> Option<String> {
    // Look for the first non-ASCII char that's likely a Nerd Font icon.
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Skip Go template expressions {{ ... }}
            if chars.peek() == Some(&'{') {
                while let Some(c) = chars.next() {
                    if c == '}' && chars.peek() == Some(&'}') {
                        chars.next();
                        break;
                    }
                }
            }
            continue;
        }
        // Nerd Font icons are in Private Use Area (U+E000..U+F8FF) or higher
        if ch as u32 >= 0xE000 {
            return Some(ch.to_string());
        }
    }
    None
}

/// Extract static text from an OMP template, stripping Go template expressions.
fn extract_static_text(template: &str) -> String {
    let mut result = String::new();
    let mut in_template = false;
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if !in_template && i + 1 < chars.len() && chars[i] == '{' && chars[i + 1] == '{' {
            in_template = true;
            i += 2;
            continue;
        }
        if in_template && i + 1 < chars.len() && chars[i] == '}' && chars[i + 1] == '}' {
            in_template = false;
            i += 2;
            continue;
        }
        if !in_template {
            // Strip OMP inline color tags: <#RRGGBB>...</> or <parentBackground>...</>
            if chars[i] == '<' {
                // Find closing >
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '>' {
                    j += 1;
                }
                if j < chars.len() {
                    let tag: String = chars[i + 1..j].iter().collect();
                    if tag.starts_with('#') || tag.starts_with('/') || tag == "parentBackground"
                        || tag.starts_with("transparent") || tag.starts_with("p:")
                    {
                        i = j + 1;
                        continue;
                    }
                }
            }
            result.push(chars[i]);
        }
        i += 1;
    }

    result.trim().to_string()
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_omp() {
        let json = r##"{
            "version": 2,
            "blocks": [{
                "type": "prompt",
                "alignment": "left",
                "segments": [{
                    "type": "path",
                    "style": "powerline",
                    "foreground": "#ffffff",
                    "background": "#FF9248",
                    "powerline_symbol": "\ue0b0",
                    "template": " \uf07b {{ .Path }} "
                }]
            }]
        }"##;
        let theme = parse(json).unwrap();
        assert_eq!(theme.left.len(), 1);
        assert_eq!(theme.left[0].name, "dir");
        assert_eq!(theme.left[0].fg.as_deref(), Some("#ffffff"));
        assert_eq!(theme.left[0].bg.as_deref(), Some("#FF9248"));
        assert_eq!(theme.left[0].trailing_char.as_deref(), Some("\u{e0b0}"));
    }

    #[test]
    fn parse_two_line_theme() {
        let json = r##"{
            "version": 2,
            "blocks": [
                {
                    "type": "prompt",
                    "alignment": "left",
                    "segments": [{"type": "path", "style": "plain", "template": "{{ .Path }}"}]
                },
                {
                    "type": "prompt",
                    "alignment": "left",
                    "newline": true,
                    "segments": [{"type": "text", "style": "plain", "template": "❯ "}]
                }
            ]
        }"##;
        let theme = parse(json).unwrap();
        assert!(theme.two_line);
        assert_eq!(theme.top.len(), 1);
        assert_eq!(theme.left.len(), 1);
        assert_eq!(theme.left[0].name, "text");
    }

    #[test]
    fn map_all_critical_types() {
        assert_eq!(map_segment_type("path"), "dir");
        assert_eq!(map_segment_type("git"), "git_branch");
        assert_eq!(map_segment_type("node"), "lang_version");
        assert_eq!(map_segment_type("python"), "lang_version");
        assert_eq!(map_segment_type("kubectl"), "kubectl_context");
        assert_eq!(map_segment_type("aws"), "aws_profile");
        assert_eq!(map_segment_type("unknown_type"), "text");
    }

    #[test]
    fn extract_icon_from_template() {
        let icon = extract_icon(" \u{f07b} {{ .Path }} ");
        assert_eq!(icon.as_deref(), Some("\u{f07b}"));
    }

    #[test]
    fn extract_static_text_strips_templates() {
        let text = extract_static_text("└─");
        assert_eq!(text, "└─");

        let text = extract_static_text("{{ if .Error }}{{ .Error }}{{ else }}{{ .Full }}{{ end }}");
        assert_eq!(text, "");

        let text = extract_static_text("\u{e718} {{ .Full }}");
        assert_eq!(text, "\u{e718}");
    }

    #[test]
    fn diamond_style_extracts_chars() {
        let json = r##"{
            "version": 2,
            "blocks": [{
                "type": "prompt",
                "alignment": "left",
                "segments": [{
                    "type": "shell",
                    "style": "diamond",
                    "leading_diamond": "\ue0b6",
                    "trailing_diamond": "\ue0b4",
                    "template": "\uf120 {{ .Name }} "
                }]
            }]
        }"##;
        let theme = parse(json).unwrap();
        assert_eq!(theme.left[0].leading_char.as_deref(), Some("\u{e0b6}"));
        assert_eq!(theme.left[0].trailing_char.as_deref(), Some("\u{e0b4}"));
    }

    #[test]
    fn transient_prompt_parsed() {
        let json = r##"{
            "version": 2,
            "blocks": [],
            "transient_prompt": {
                "template": "❯ ",
                "foreground": "#e0f8ff"
            }
        }"##;
        let theme = parse(json).unwrap();
        assert!(theme.transient.is_some());
        assert_eq!(theme.transient.unwrap().template, "❯");
    }

    #[test]
    fn palette_references_resolved() {
        let json = r##"{
            "version": 2,
            "palette": { "bg": "#1a1b26" },
            "blocks": [{
                "type": "prompt",
                "alignment": "left",
                "segments": [{
                    "type": "path",
                    "style": "plain",
                    "background": "p:bg",
                    "template": "{{ .Path }}"
                }]
            }]
        }"##;
        let theme = parse(json).unwrap();
        assert_eq!(theme.left[0].bg.as_deref(), Some("#1a1b26"));
    }

    #[test]
    fn right_block_goes_to_top_right() {
        let json = r##"{
            "version": 2,
            "blocks": [
                {
                    "type": "prompt",
                    "alignment": "left",
                    "segments": [{"type": "path", "style": "plain", "template": "{{ .Path }}"}]
                },
                {
                    "type": "prompt",
                    "alignment": "right",
                    "segments": [{"type": "time", "style": "plain", "template": "{{ .CurrentDate }}"}]
                }
            ]
        }"##;
        let theme = parse(json).unwrap();
        assert_eq!(theme.left.len(), 1);
        assert_eq!(theme.top_right.len(), 1);
        assert_eq!(theme.top_right[0].name, "time");
    }
}
