use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct UsernameConfig {
    /// Override format: `"user"` (default), `"user@host"`, or `"full"`.
    /// Use the hostname segment for host display — this field is reserved for future use.
    #[serde(default)]
    show_always: bool,
}

pub struct UsernameSegment;

impl Segment for UsernameSegment {
    fn name(&self) -> &'static str {
        "username"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: UsernameConfig = config.clone().try_into().unwrap_or_default();
        let user = ctx.env.get("USER")?.clone();
        if user.is_empty() {
            return None;
        }
        // Show as bold-red when running as root (UID=0).
        let uid_zero = ctx
            .env
            .get("UID")
            .map(|v| v == "0")
            .unwrap_or(false);
        let text = if uid_zero {
            format!("\x1b[1;31m{}\x1b[0m", user)
        } else {
            user
        };
        let _ = cfg.show_always; // reserved for future visibility control
        Some(RenderedSegment::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_env(pairs: &[(&str, &str)]) -> RenderContext {
        let env = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env,
        }
    }

    #[test]
    fn renders_username_from_env() {
        let ctx = ctx_with_env(&[("USER", "alice")]);
        let r = UsernameSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "alice");
    }

    #[test]
    fn root_renders_bold_red() {
        let ctx = ctx_with_env(&[("USER", "root"), ("UID", "0")]);
        let r = UsernameSegment.render(&empty_config(), &ctx).unwrap();
        assert!(r.text.contains("\x1b[1;31m"), "expected bold-red ANSI: {}", r.text);
        assert!(r.text.contains("root"));
    }

    #[test]
    fn non_root_no_ansi() {
        let ctx = ctx_with_env(&[("USER", "alice"), ("UID", "1000")]);
        let r = UsernameSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "alice");
    }

    #[test]
    fn hidden_when_user_missing() {
        let ctx = ctx_with_env(&[]);
        let r = UsernameSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }
}
