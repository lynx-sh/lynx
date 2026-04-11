/// Custom template segment — renders a TOML-defined template string using
/// RenderContext data. No I/O, no shell execution.
///
/// # TOML usage
///
/// ```toml
/// [segments.left]
/// order = ["custom_greeting", "dir"]
///
/// [segment.custom_greeting]
/// template = "hello ${env.USER}"
/// color = { fg = "blue" }
/// show_in = ["interactive"]
/// ```
///
/// # Template syntax
///
/// Simple variables (`$name`):
/// - `$cwd`         — current working directory
/// - `$context`     — shell context: `interactive`, `agent`, or `minimal`
/// - `$last_cmd_ms` — last command duration in ms, or empty if not set
///
/// Dotted paths (`${section.key}`):
/// - `${env.VAR}`              — environment variable from the context snapshot
/// - `${cache.PLUGIN.FIELD}`   — field from a plugin's JSON state cache
///
/// Unknown variables and missing cache fields resolve to an empty string.
/// A literal `$$` produces a single `$`.
use crate::segment::{RenderContext, RenderedSegment, Segment};

pub struct CustomSegment;

impl Segment for CustomSegment {
    fn name(&self) -> &'static str {
        // Registered as "custom"; evaluator routes "custom_*" names here.
        "custom"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let template = config.get("template")?.as_str()?;
        if template.is_empty() {
            return None;
        }
        let text = render_custom_template(template, ctx);
        if text.is_empty() {
            return None;
        }
        Some(RenderedSegment::new(text))
    }
}

/// Render a custom template string against a `RenderContext`.
///
/// Supports two variable forms:
/// - `$name` — simple identifier (alphanumeric + underscore)
/// - `${dotted.path}` — dot-separated path for env and cache lookups
///
/// Unknown identifiers and missing paths resolve to `""`.
pub fn render_custom_template(template: &str, ctx: &RenderContext) -> String {
    let context_str = match ctx.shell_context {
        lynx_core::types::Context::Interactive => "interactive",
        lynx_core::types::Context::Agent => "agent",
        lynx_core::types::Context::Minimal => "minimal",
    };

    let last_cmd_ms = ctx
        .last_cmd_ms
        .map(|ms| ms.to_string())
        .unwrap_or_default();

    let mut result = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '$' {
            result.push(chars[i]);
            i += 1;
            continue;
        }

        // Escaped `$$` → literal `$`
        if i + 1 < chars.len() && chars[i + 1] == '$' {
            result.push('$');
            i += 2;
            continue;
        }

        // Brace form: `${dotted.path}`
        if i + 1 < chars.len() && chars[i + 1] == '{' {
            let start = i + 2;
            if let Some(end_offset) = chars[start..].iter().position(|&c| c == '}') {
                let path: String = chars[start..start + end_offset].iter().collect();
                result.push_str(&resolve_dotted(path.trim(), ctx));
                i = start + end_offset + 1;
                continue;
            }
            // Unclosed brace — emit literally and advance past `$`
            result.push('$');
            i += 1;
            continue;
        }

        // Simple form: `$name` (alphanumeric + underscore)
        let name_start = i + 1;
        let mut j = name_start;
        while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
            j += 1;
        }
        let name: String = chars[name_start..j].iter().collect();
        if name.is_empty() {
            result.push('$');
            i += 1;
            continue;
        }

        let value = match name.as_str() {
            "cwd" => ctx.cwd.as_str(),
            "context" => context_str,
            "last_cmd_ms" => last_cmd_ms.as_str(),
            _ => "",
        };
        result.push_str(value);
        i = j;
    }

    result
}

/// Resolve a dot-separated path against the RenderContext.
///
/// Supported forms:
/// - `env.VAR`              → `ctx.env["VAR"]`
/// - `cache.PLUGIN.FIELD`   → `ctx.cache["PLUGIN"]["FIELD"]` (string values only)
fn resolve_dotted(path: &str, ctx: &RenderContext) -> String {
    let parts: Vec<&str> = path.splitn(3, '.').collect();
    match parts.as_slice() {
        ["env", var] => ctx.env.get(*var).cloned().unwrap_or_default(),
        ["cache", plugin, field] => ctx
            .cache
            .get(*plugin)
            .and_then(|v| v.get(*field))
            .and_then(|v| {
                // Support both string values and numeric/bool values coerced to string.
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    Some(v.to_string())
                }
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_core::types::Context;

    fn ctx_with(
        cwd: &str,
        env: &[(&str, &str)],
        cache: &[(&str, serde_json::Value)],
    ) -> RenderContext {
        RenderContext {
            cwd: cwd.to_string(),
            shell_context: Context::Interactive,
            last_cmd_ms: None,
            cache: cache.iter().cloned().map(|(k, v)| (k.to_string(), v)).collect(),
            env: env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    fn cfg(template: &str) -> toml::Value {
        let mut map = toml::map::Map::new();
        map.insert("template".to_string(), toml::Value::String(template.to_string()));
        toml::Value::Table(map)
    }

    #[test]
    fn simple_cwd_var() {
        let ctx = ctx_with("/home/user/code", &[], &[]);
        assert_eq!(render_custom_template("in $cwd", &ctx), "in /home/user/code");
    }

    #[test]
    fn context_var() {
        let ctx = ctx_with("/", &[], &[]);
        assert_eq!(render_custom_template("ctx:$context", &ctx), "ctx:interactive");
    }

    #[test]
    fn env_var_dotted() {
        let ctx = ctx_with("/", &[("USER", "alice"), ("HOST", "box")], &[]);
        assert_eq!(
            render_custom_template("${env.USER}@${env.HOST}", &ctx),
            "alice@box"
        );
    }

    #[test]
    fn cache_dotted() {
        let ctx = ctx_with(
            "/",
            &[],
            &[("git", serde_json::json!({"branch": "main", "dirty": "1"}))],
        );
        assert_eq!(
            render_custom_template("branch:${cache.git.branch}", &ctx),
            "branch:main"
        );
    }

    #[test]
    fn missing_env_var_is_empty() {
        let ctx = ctx_with("/", &[], &[]);
        assert_eq!(render_custom_template("${env.UNDEFINED}", &ctx), "");
    }

    #[test]
    fn missing_cache_field_is_empty() {
        let ctx = ctx_with("/", &[], &[]);
        assert_eq!(render_custom_template("${cache.git.branch}", &ctx), "");
    }

    #[test]
    fn unknown_simple_var_is_empty() {
        let ctx = ctx_with("/", &[], &[]);
        assert_eq!(render_custom_template("$undefined_var", &ctx), "");
    }

    #[test]
    fn escaped_dollar() {
        let ctx = ctx_with("/", &[], &[]);
        assert_eq!(render_custom_template("price: $$5", &ctx), "price: $5");
    }

    #[test]
    fn mixed_template() {
        let ctx = ctx_with(
            "/projects",
            &[("USER", "bob")],
            &[("git", serde_json::json!({"branch": "feat"}))],
        );
        let tmpl = "${env.USER} | $cwd | ${cache.git.branch}";
        assert_eq!(
            render_custom_template(tmpl, &ctx),
            "bob | /projects | feat"
        );
    }

    #[test]
    fn segment_render_returns_none_when_no_template() {
        let seg = CustomSegment;
        let cfg_no_template = toml::Value::Table(toml::map::Map::new());
        let ctx = ctx_with("/", &[], &[]);
        assert!(seg.render(&cfg_no_template, &ctx).is_none());
    }

    #[test]
    fn segment_render_returns_none_when_empty_result() {
        let seg = CustomSegment;
        // Template only references a missing env var → empty string → None
        let ctx = ctx_with("/", &[], &[]);
        assert!(seg.render(&cfg("${env.MISSING}"), &ctx).is_none());
    }

    #[test]
    fn segment_render_returns_rendered_text() {
        let seg = CustomSegment;
        let ctx = ctx_with("/home", &[("USER", "carol")], &[]);
        let result = seg.render(&cfg("hi ${env.USER}"), &ctx).unwrap();
        assert_eq!(result.text, "hi carol");
    }

    #[test]
    fn last_cmd_ms_var() {
        let mut ctx = ctx_with("/", &[], &[]);
        ctx.last_cmd_ms = Some(123);
        assert_eq!(render_custom_template("took $last_cmd_ms ms", &ctx), "took 123 ms");
    }

    #[test]
    fn last_cmd_ms_missing_is_empty() {
        let ctx = ctx_with("/", &[], &[]);
        assert_eq!(render_custom_template("[$last_cmd_ms]", &ctx), "[]");
    }
}
