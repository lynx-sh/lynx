use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct VenvConfig {
    /// Symbol prepended to the env name. Default: empty.
    symbol: Option<String>,
}

pub struct VenvSegment;

impl Segment for VenvSegment {
    fn name(&self) -> &'static str {
        "venv"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: VenvConfig = config.clone().try_into().unwrap_or_default();
        let path = ctx.env.get("VIRTUAL_ENV")?;
        if path.is_empty() {
            return None;
        }
        // Show only the basename of the venv path.
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path.as_str());
        let text = match cfg.symbol {
            Some(ref sym) => format!("{sym} {name}"),
            None => name.to_string(),
        };
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
    fn hidden_when_virtual_env_missing() {
        let ctx = ctx_with_env(&[]);
        let r = VenvSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_basename_of_path() {
        let ctx = ctx_with_env(&[("VIRTUAL_ENV", "/home/user/.venv/myproject")]);
        let r = VenvSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "myproject");
    }

    #[test]
    fn symbol_prepended() {
        let cfg: toml::Value = toml::from_str(r#"symbol = "🐍""#).unwrap();
        let ctx = ctx_with_env(&[("VIRTUAL_ENV", "/home/user/.venv/myproject")]);
        let r = VenvSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "🐍 myproject");
    }
}
