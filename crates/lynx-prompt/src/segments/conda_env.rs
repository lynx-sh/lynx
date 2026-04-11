use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct CondaEnvConfig {
    /// When true, hide the segment when the active environment is "base". Default: true.
    #[serde(default = "default_hide_base")]
    hide_base: bool,
    /// Symbol prepended to the env name. Default: empty.
    symbol: Option<String>,
}

fn default_hide_base() -> bool {
    true
}

pub struct CondaEnvSegment;

impl Segment for CondaEnvSegment {
    fn name(&self) -> &'static str {
        "conda_env"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: CondaEnvConfig = config.clone().try_into().unwrap_or_default();
        let name = ctx.env.get("CONDA_DEFAULT_ENV")?;
        if name.is_empty() {
            return None;
        }
        if cfg.hide_base && name == "base" {
            return None;
        }
        let text = match cfg.symbol {
            Some(ref sym) => format!("{} {}", sym, name),
            None => name.clone(),
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
    fn hidden_when_env_missing() {
        let ctx = ctx_with_env(&[]);
        let r = CondaEnvSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_base_by_default() {
        let ctx = ctx_with_env(&[("CONDA_DEFAULT_ENV", "base")]);
        let r = CondaEnvSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_base_when_hide_base_false() {
        let cfg: toml::Value = toml::from_str("hide_base = false").unwrap();
        let ctx = ctx_with_env(&[("CONDA_DEFAULT_ENV", "base")]);
        let r = CondaEnvSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "base");
    }

    #[test]
    fn shows_non_base_env() {
        let ctx = ctx_with_env(&[("CONDA_DEFAULT_ENV", "myenv")]);
        let r = CondaEnvSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "myenv");
    }

    #[test]
    fn symbol_prepended() {
        let cfg: toml::Value = toml::from_str(r#"symbol = "C""#).unwrap();
        let ctx = ctx_with_env(&[("CONDA_DEFAULT_ENV", "myenv")]);
        let r = CondaEnvSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "C myenv");
    }
}
