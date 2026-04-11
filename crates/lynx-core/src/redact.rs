/// Replace values associated with secret-shaped keys with `[REDACTED]`.
///
/// Secret keys are those whose name (or last segment after `_`) ends with one of:
/// `_KEY`, `_TOKEN`, `_SECRET`, `_PASSWORD`, `_CREDENTIAL`, `_PRIVATE`
/// (case-insensitive, suffix-only — `KEYBOARD` is NOT matched).
///
/// Handles four input formats:
/// - `KEY=value`  (plain env / shell assignment)
/// - `export KEY=value`
/// - TOML  `key = "value"`
/// - JSON  `"key": "value"`
pub fn redact(s: &str) -> String {
    s.lines()
        .map(|line| redact_line(line))
        .collect::<Vec<_>>()
        .join("\n")
        // Preserve trailing newline if original had one.
        + if s.ends_with('\n') { "\n" } else { "" }
}

/// Returns true if the env key name looks like it holds a secret.
/// Used at profile parse time to warn before storing secrets in profiles.
pub fn looks_like_secret_value(key: &str, _value: &str) -> bool {
    is_secret_key(key)
}

fn is_secret_key(key: &str) -> bool {
    let upper = key.trim().to_uppercase();
    const SUFFIXES: &[&str] = &[
        "_KEY", "_TOKEN", "_SECRET", "_PASSWORD", "_CREDENTIAL", "_PRIVATE",
    ];
    SUFFIXES.iter().any(|s| upper.ends_with(s))
}

fn redact_line(line: &str) -> String {
    let trimmed = line.trim_start();

    // export KEY=value  or  KEY=value
    if let Some(rest) = trimmed.strip_prefix("export ") {
        if let Some(redacted) = try_redact_kv(rest) {
            let indent = &line[..line.len() - trimmed.len()];
            return format!("{indent}export {redacted}");
        }
        return line.to_string();
    }

    // Plain KEY=value
    if let Some(redacted) = try_redact_kv(trimmed) {
        let indent = &line[..line.len() - trimmed.len()];
        return format!("{indent}{redacted}");
    }

    // TOML: key = "value"  or  key = 'value'
    if let Some(redacted) = try_redact_toml(trimmed) {
        let indent = &line[..line.len() - trimmed.len()];
        return format!("{indent}{redacted}");
    }

    // JSON: "key": "value"
    if let Some(redacted) = try_redact_json(trimmed) {
        let indent = &line[..line.len() - trimmed.len()];
        return format!("{indent}{redacted}");
    }

    line.to_string()
}

/// Try to match `KEY=value` and redact if key is secret.
fn try_redact_kv(s: &str) -> Option<String> {
    let eq = s.find('=')?;
    let key = &s[..eq];
    if !is_valid_identifier(key) || !is_secret_key(key) {
        return None;
    }
    Some(format!("{key}=[REDACTED]"))
}

/// Try to match TOML `key = "value"` or `key = 'value'`.
fn try_redact_toml(s: &str) -> Option<String> {
    // key = "..." or key = '...'
    let eq = s.find('=')?;
    let key = s[..eq].trim();
    if !is_valid_identifier(key) || !is_secret_key(key) {
        return None;
    }
    let after_eq = s[eq + 1..].trim();
    let is_quoted = (after_eq.starts_with('"') && after_eq.ends_with('"'))
        || (after_eq.starts_with('\'') && after_eq.ends_with('\''));
    if !is_quoted {
        return None;
    }
    let q = &after_eq[..1];
    Some(format!("{key} = {q}[REDACTED]{q}"))
}

/// Try to match JSON `"key": "value"`.
fn try_redact_json(s: &str) -> Option<String> {
    // "key": "value"  (handles optional trailing comma)
    if !s.starts_with('"') {
        return None;
    }
    let close = s[1..].find('"')? + 1;
    let key = &s[1..close];
    if !is_secret_key(key) {
        return None;
    }
    let after_key = s[close + 1..].trim_start();
    if !after_key.starts_with(':') {
        return None;
    }
    let after_colon = after_key[1..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let trail = if after_colon.ends_with(',') { "," } else { "" };
    Some(format!("\"{key}\": \"[REDACTED]\"{trail}"))
}

fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_secret_key_redacted() {
        assert_eq!(redact("AWS_SECRET_ACCESS_KEY=abc123"), "AWS_SECRET_ACCESS_KEY=[REDACTED]");
    }

    #[test]
    fn keyboard_not_redacted() {
        assert_eq!(redact("KEYBOARD=qwerty"), "KEYBOARD=qwerty");
    }

    #[test]
    fn export_token_redacted() {
        assert_eq!(
            redact("export API_TOKEN=xyz"),
            "export API_TOKEN=[REDACTED]"
        );
    }

    #[test]
    fn toml_value_redacted() {
        assert_eq!(
            redact(r#"api_key = "super_secret""#),
            r#"api_key = "[REDACTED]""#
        );
    }

    #[test]
    fn toml_single_quote_redacted() {
        assert_eq!(
            redact("db_password = 'hunter2'"),
            "db_password = '[REDACTED]'"
        );
    }

    #[test]
    fn json_value_redacted() {
        assert_eq!(
            redact(r#"  "api_token": "abc","#),
            r#"  "api_token": "[REDACTED]","#
        );
    }

    #[test]
    fn json_no_comma_redacted() {
        assert_eq!(
            redact(r#"  "github_private": "key""#),
            r#"  "github_private": "[REDACTED]""#
        );
    }

    #[test]
    fn normal_key_not_redacted() {
        assert_eq!(redact("USERNAME=alice"), "USERNAME=alice");
        assert_eq!(redact("HOME=/home/alice"), "HOME=/home/alice");
    }

    #[test]
    fn multiline_redacts_only_secrets() {
        let input = "USERNAME=alice\nAPI_KEY=secret\nHOME=/home\n";
        let out = redact(input);
        assert!(out.contains("USERNAME=alice"));
        assert!(out.contains("API_KEY=[REDACTED]"));
        assert!(out.contains("HOME=/home"));
    }

    #[test]
    fn all_suffixes_covered() {
        for suffix in ["_KEY", "_TOKEN", "_SECRET", "_PASSWORD", "_CREDENTIAL", "_PRIVATE"] {
            let line = format!("MY{suffix}=value");
            let out = redact(&line);
            assert!(out.contains("[REDACTED]"), "suffix {suffix} not redacted: {out}");
        }
    }

    #[test]
    fn case_insensitive_key() {
        assert_eq!(redact("my_secret=val"), "my_secret=[REDACTED]");
        assert_eq!(redact("MY_SECRET=val"), "MY_SECRET=[REDACTED]");
    }

    #[test]
    fn non_suffix_not_redacted() {
        // SECRETIVE does not end with _SECRET — should not be redacted.
        // (no underscore before SECRET here)
        assert_eq!(redact("SECRETIVE=value"), "SECRETIVE=value");
    }

    #[test]
    fn pure_function_no_side_effects() {
        let s = "API_KEY=x";
        let a = redact(s);
        let b = redact(s);
        assert_eq!(a, b);
    }
}
