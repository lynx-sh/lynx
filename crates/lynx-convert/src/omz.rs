use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Intermediate representation of a parsed OMZ theme.
#[derive(Debug, Clone, Default)]
pub struct OmzTheme {
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub colors: HashMap<String, SegColor>,
    pub two_line: bool,
    pub tier: Tier,
    pub notes: Vec<String>,
}

/// Segment color extracted from OMZ color annotations.
#[derive(Debug, Clone, Default)]
pub struct SegColor {
    pub fg: Option<String>,
    pub bold: bool,
}

/// Complexity tier of the OMZ theme.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Tier {
    /// Simple PROMPT= assignment with % tokens and $(func) calls.
    #[default]
    Simple,
    /// Uses ZSH_THEME_* variables for customization.
    Customized,
    /// Agnoster-style with prompt_segment() functions — partial conversion only.
    Agnoster,
}

/// Parse an OMZ .zsh-theme file content into an IR.
pub fn parse(content: &str) -> OmzTheme {
    let mut theme = OmzTheme::default();

    // Detect agnoster-style themes.
    if content.contains("prompt_segment") || content.contains("build_prompt") {
        theme.tier = Tier::Agnoster;
        theme.notes.push(
            "agnoster-style theme — segment order approximated, colors manually tuned".to_string(),
        );
        // Best-effort: extract common segments from build_prompt.
        parse_agnoster_segments(content, &mut theme);
        return theme;
    }

    // Extract PROMPT and RPROMPT assignments.
    let prompt_str = extract_assignment(content, "PROMPT");
    let rprompt_str = extract_assignment(content, "RPROMPT");

    // Detect two-line prompts — check both parsed PROMPT and raw content.
    if let Some(ref p) = prompt_str {
        theme.two_line = p.contains("╭") || p.contains("╰") || p.contains("\\n") || p.contains("$'\\n'");
    }
    if !theme.two_line {
        theme.two_line = content.contains("╭") || content.contains("╰");
    }

    // Parse segments from PROMPT.
    if let Some(ref p) = prompt_str {
        theme.left = extract_segments(p);
        extract_colors(p, &mut theme.colors);
    }

    // Parse segments from RPROMPT.
    if let Some(ref p) = rprompt_str {
        theme.right = extract_segments(p);
        extract_colors(p, &mut theme.colors);
    }

    // Check for ZSH_THEME_* customization.
    if content.contains("ZSH_THEME_")
        && theme.tier == Tier::Simple {
            theme.tier = Tier::Customized;
        }

    theme
}

/// Extract the value of a variable assignment like PROMPT='...' or PROMPT="...".
/// Also handles PROMPT+= continuation and $'...' ANSI-C quoting.
fn extract_assignment(content: &str, var: &str) -> Option<String> {
    let mut result = String::new();

    // Match VAR= and VAR+= lines.
    let re = Regex::new(&format!(
        r#"(?m)^{var}\s*\+?=\s*\$?['"](.+?)['"]"#
    )).ok()?;

    for caps in re.captures_iter(content) {
        result.push_str(&caps[1]);
    }

    if result.is_empty() {
        // Try multi-line (dot matches newline).
        let re2 = Regex::new(&format!(
            r#"(?ms)^{var}\s*\+?=\s*\$?['"](.+?)['"\n]"#
        )).ok()?;
        for caps in re2.captures_iter(content) {
            result.push_str(&caps[1]);
        }
    }

    if result.is_empty() { None } else { Some(result) }
}

/// Map OMZ prompt tokens and function calls to Lynx segment names.
fn extract_segments(prompt: &str) -> Vec<String> {
    let mut segs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mappings: &[(&str, &str)] = &[
        // % tokens
        ("%n", "username"),
        ("%m", "hostname"),
        ("%M", "hostname"),
        ("%~", "dir"),
        ("%c", "dir"),
        ("%/", "dir"),
        ("%T", "time"),
        ("%*", "time"),
        ("%t", "time"),
        ("%?", "exit_code"),
        ("%!", "hist_number"),
        ("%#", "prompt_char"),
        // Function calls
        ("$(git_prompt_info)", "git_branch"),
        ("$(git_prompt_status)", "git_status"),
        ("$(ruby_prompt_info)", "ruby_version"),
        ("$(virtualenv_prompt_info)", "venv"),
        ("$(nvm_prompt_info)", "node_version"),
        ("$(conda_prompt_info)", "conda_env"),
    ];

    for (token, seg) in mappings {
        if prompt.contains(token) && seen.insert(*seg) {
            segs.push(seg.to_string());
        }
    }

    // %D{...} time format.
    if Regex::new(r"%D\{").ok().map(|r| r.is_match(prompt)).unwrap_or(false) && seen.insert("time") {
        segs.push("time".to_string());
    }

    // ❯ or similar prompt_char at end.
    if (prompt.ends_with("❯ ") || prompt.ends_with("$ ") || prompt.ends_with("> "))
        && seen.insert("prompt_char")
    {
        segs.push("prompt_char".to_string());
    }

    segs
}

/// Extract color annotations from OMZ prompt strings.
fn extract_colors(prompt: &str, colors: &mut HashMap<String, SegColor>) {
    // Pattern: $fg[color] or $fg_bold[color] followed by segment content.
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\$fg(?:_bold)?\[(\w+)\]").expect("static regex"));
    static BOLD_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\$fg_bold\[(\w+)\]").expect("static regex"));

    for caps in RE.captures_iter(prompt) {
        let color_name = &caps[1];
        let full_match = caps.get(0).expect("capture group 0 always exists");
        let is_bold = BOLD_RE.is_match(full_match.as_str());

        // Try to associate with the next segment token.
        let pos = full_match.end();
        let rest = &prompt[pos..];

        let seg_name = identify_next_segment(rest);
        if let Some(name) = seg_name {
            colors.insert(
                name,
                SegColor {
                    fg: Some(color_name.to_string()),
                    bold: is_bold,
                },
            );
        }
    }
}

/// Identify which segment follows a color annotation.
fn identify_next_segment(text: &str) -> Option<String> {
    let trimmed = text.trim_start_matches(['}', '%', '{', ' ']);
    let mappings: &[(&str, &str)] = &[
        ("%n", "username"),
        ("%m", "hostname"),
        ("%~", "dir"),
        ("%c", "dir"),
        ("$(git_prompt_info)", "git_branch"),
    ];
    for (token, seg) in mappings {
        if trimmed.starts_with(token) {
            return Some(seg.to_string());
        }
    }
    None
}

/// Best-effort segment extraction for agnoster-style themes.
fn parse_agnoster_segments(content: &str, theme: &mut OmzTheme) {
    let common = [
        "dir", "git_branch", "git_status", "venv", "prompt_char",
    ];
    // Look for prompt_segment calls to determine order.
    if content.contains("prompt_dir") || content.contains("prompt_context") {
        for seg in &common {
            theme.left.push(seg.to_string());
        }
    } else {
        // Fallback: use common agnoster order.
        theme.left = vec![
            "username".to_string(),
            "dir".to_string(),
            "git_branch".to_string(),
            "git_status".to_string(),
            "prompt_char".to_string(),
        ];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROBBYRUSSELL: &str = r#"PROMPT="%(?:%{$fg_bold[green]%}❯ :%{$fg_bold[red]%}❯ )"
PROMPT+=' %{$fg[cyan]%}%c%{$reset_color%} $(git_prompt_info)'

ZSH_THEME_GIT_PROMPT_PREFIX="%{$fg_bold[blue]%}git:(%{$fg[red]%}"
ZSH_THEME_GIT_PROMPT_SUFFIX="%{$reset_color%} "
ZSH_THEME_GIT_PROMPT_DIRTY="%{$fg[blue]%}) %{$fg[yellow]%}✗"
ZSH_THEME_GIT_PROMPT_CLEAN="%{$fg[blue]%})"
"#;

    const CANDY: &str = r#"PROMPT=$'%{$fg_bold[green]%}%n@%m %{$fg[blue]%}%D{[%X]} %{$reset_color%}%{$fg[white]%}[%~]%{$reset_color%} $(git_prompt_info)\
%{$fg[blue]%}→%{$reset_color%} '"#;

    const BIRA: &str = r#"PROMPT="╭─%{$fg_bold[green]%}%n@%m%{$reset_color%} %{$fg[blue]%}%~%{$reset_color%} $(git_prompt_info)
╰─%B$%b "#;

    const GNZH: &str = r#"PROMPT='%{$fg_bold[green]%}%n@%m%{$reset_color%} %{$fg[blue]%}%~%{$reset_color%} $(git_prompt_info)$(git_prompt_status)%{$reset_color%}
%B❯%b '"#;

    #[test]
    fn parse_robbyrussell() {
        let theme = parse(ROBBYRUSSELL);
        assert!(theme.left.contains(&"dir".to_string()), "expected dir: {:?}", theme.left);
        assert!(theme.left.contains(&"git_branch".to_string()), "expected git_branch: {:?}", theme.left);
        assert_eq!(theme.tier, Tier::Customized);
    }

    #[test]
    fn parse_candy() {
        let theme = parse(CANDY);
        assert!(theme.left.contains(&"username".to_string()), "expected username: {:?}", theme.left);
        assert!(theme.left.contains(&"hostname".to_string()), "expected hostname: {:?}", theme.left);
        assert!(theme.left.contains(&"time".to_string()), "expected time: {:?}", theme.left);
        assert!(theme.left.contains(&"dir".to_string()), "expected dir: {:?}", theme.left);
        assert!(theme.left.contains(&"git_branch".to_string()), "expected git_branch: {:?}", theme.left);
    }

    #[test]
    fn parse_bira_detects_two_line() {
        let theme = parse(BIRA);
        assert!(theme.two_line, "expected two-line detection");
        assert!(theme.left.contains(&"username".to_string()));
        assert!(theme.left.contains(&"dir".to_string()));
    }

    #[test]
    fn parse_gnzh() {
        let theme = parse(GNZH);
        assert!(theme.left.contains(&"username".to_string()));
        assert!(theme.left.contains(&"hostname".to_string()));
        assert!(theme.left.contains(&"dir".to_string()));
        assert!(theme.left.contains(&"git_branch".to_string()));
        assert!(theme.left.contains(&"git_status".to_string()));
    }

    #[test]
    fn agnoster_style_detected() {
        let content = "prompt_segment() { ... }\nbuild_prompt() { prompt_dir; prompt_git; }";
        let theme = parse(content);
        assert_eq!(theme.tier, Tier::Agnoster);
        assert!(!theme.notes.is_empty());
    }
}
