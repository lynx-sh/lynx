use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct HostnameConfig {
    /// When true, only show hostname during SSH sessions. Default: true.
    #[serde(default = "default_show_when_ssh")]
    show_when_ssh: bool,
}

fn default_show_when_ssh() -> bool {
    true
}

pub struct HostnameSegment;

impl Segment for HostnameSegment {
    fn name(&self) -> &'static str {
        "hostname"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: HostnameConfig = config.clone().try_into().unwrap_or_default();
        let hostname = ctx.env.get("HOSTNAME")?.clone();
        if hostname.is_empty() {
            return None;
        }
        if cfg.show_when_ssh {
            // Only show when inside an SSH session.
            let is_ssh = ctx.env.contains_key("SSH_CONNECTION") || ctx.env.contains_key("SSH_TTY");
            if !is_ssh {
                return None;
            }
        }
        Some(RenderedSegment::new(hostname))
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
    fn hidden_outside_ssh_by_default() {
        let ctx = ctx_with_env(&[("HOSTNAME", "mybox")]);
        let r = HostnameSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_during_ssh_connection() {
        let ctx = ctx_with_env(&[("HOSTNAME", "mybox"), ("SSH_CONNECTION", "1.2.3.4 22 5.6.7.8 12345")]);
        let r = HostnameSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "mybox");
    }

    #[test]
    fn shows_during_ssh_tty() {
        let ctx = ctx_with_env(&[("HOSTNAME", "mybox"), ("SSH_TTY", "/dev/pts/0")]);
        let r = HostnameSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "mybox");
    }

    #[test]
    fn show_when_ssh_false_always_shows() {
        let cfg: toml::Value = toml::from_str("show_when_ssh = false").unwrap();
        let ctx = ctx_with_env(&[("HOSTNAME", "mybox")]);
        let r = HostnameSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "mybox");
    }

    #[test]
    fn hidden_when_hostname_missing() {
        let ctx = ctx_with_env(&[("SSH_CONNECTION", "x")]);
        let r = HostnameSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }
}
