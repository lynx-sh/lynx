use std::collections::HashMap;

use lynx_core::error::{LynxError, Result};

/// Substitute `{{TOKEN}}` placeholders in `template` using `values`.
///
/// Rules:
/// - `{{TOKEN}}` → replaced with `values["TOKEN"]`
/// - Unknown token → `LynxError::Template` error (not silent empty string)
/// - `\{{literal}}` → passes through as `{{literal}}`
/// - Nested `{{` without matching `}}` → clear error
pub fn render(template: &str, values: &HashMap<String, String>) -> Result<String> {
    let mut out = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Escape sequence: \{{ → literal {{
        if chars[i] == '\\' && i + 2 < chars.len() && chars[i + 1] == '{' && chars[i + 2] == '{' {
            out.push('{');
            out.push('{');
            i += 3;
            continue;
        }

        // Start of a token: {{
        if chars[i] == '{' && i + 1 < chars.len() && chars[i + 1] == '{' {
            // Find closing }}
            let start = i + 2;
            let mut j = start;
            loop {
                if j >= chars.len() {
                    return Err(LynxError::Config(
                        "template: unclosed {{ — missing }}".to_string(),
                    ));
                }
                // Nested {{ is not supported
                if chars[j] == '{' && j + 1 < chars.len() && chars[j + 1] == '{' {
                    return Err(LynxError::Config(
                        "template: nested {{ is not supported".to_string(),
                    ));
                }
                if chars[j] == '}' && j + 1 < chars.len() && chars[j + 1] == '}' {
                    break;
                }
                j += 1;
            }

            let token: String = chars[start..j].iter().collect();
            let token = token.trim();

            match values.get(token) {
                Some(v) => out.push_str(v),
                None => {
                    return Err(LynxError::Config(format!(
                        "template: unknown token '{{{{{}}}}}' — no value provided",
                        token
                    )));
                }
            }

            i = j + 2; // skip }}
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vals(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn basic_substitution() {
        let result = render("Hello, {{NAME}}!", &vals(&[("NAME", "Lynx")])).unwrap();
        assert_eq!(result, "Hello, Lynx!");
    }

    #[test]
    fn unknown_token_errors() {
        let err = render("{{FOO}}", &vals(&[])).unwrap_err();
        assert!(err.to_string().contains("FOO"));
    }

    #[test]
    fn escape_passes_through() {
        let result = render(r"\{{literal}}", &vals(&[])).unwrap();
        assert_eq!(result, "{{literal}}");
    }

    #[test]
    fn unclosed_brace_errors() {
        let err = render("{{ unclosed", &vals(&[])).unwrap_err();
        assert!(err.to_string().contains("unclosed"));
    }

    #[test]
    fn nested_braces_errors() {
        let err = render("{{ {{nested}} }}", &vals(&[("nested", "x")])).unwrap_err();
        assert!(err.to_string().contains("nested"));
    }

    #[test]
    fn multiple_tokens() {
        let result = render("{{A}} and {{B}}", &vals(&[("A", "foo"), ("B", "bar")])).unwrap();
        assert_eq!(result, "foo and bar");
    }

    #[test]
    fn no_tokens_passes_through() {
        let result = render("plain text", &vals(&[])).unwrap();
        assert_eq!(result, "plain text");
    }

    #[test]
    fn whitespace_trimmed_in_token_name() {
        let result = render("{{ NAME }}", &vals(&[("NAME", "trimmed")])).unwrap();
        assert_eq!(result, "trimmed");
    }
}
