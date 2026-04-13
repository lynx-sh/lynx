use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct BackgroundJobsConfig {
    /// Symbol prepended to the job count. Default: "⚙".
    symbol: Option<String>,
}

pub struct BackgroundJobsSegment;

impl Segment for BackgroundJobsSegment {
    fn name(&self) -> &'static str {
        "background_jobs"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: BackgroundJobsConfig = config.clone().try_into().unwrap_or_default();
        let jobs_str = ctx.env.get("LYNX_BG_JOBS")?;
        let jobs: u32 = jobs_str.parse().unwrap_or(0);
        if jobs == 0 {
            return None;
        }
        let symbol = cfg.symbol.unwrap_or_else(|| "⚙".to_string());
        Some(RenderedSegment::new(format!("{symbol} {jobs}")))
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
    fn hidden_when_zero_jobs() {
        let ctx = ctx_with_env(&[("LYNX_BG_JOBS", "0")]);
        let r = BackgroundJobsSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_env_missing() {
        let ctx = ctx_with_env(&[]);
        let r = BackgroundJobsSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_job_count() {
        let ctx = ctx_with_env(&[("LYNX_BG_JOBS", "3")]);
        let r = BackgroundJobsSegment.render(&empty_config(), &ctx).unwrap();
        assert!(r.text.contains('3'), "expected count: {}", r.text);
        assert!(r.text.contains('⚙'), "expected default symbol: {}", r.text);
    }

    #[test]
    fn custom_symbol() {
        let cfg: toml::Value = toml::from_str(r#"symbol = "&""#).unwrap();
        let ctx = ctx_with_env(&[("LYNX_BG_JOBS", "2")]);
        let r = BackgroundJobsSegment.render(&cfg, &ctx).unwrap();
        assert!(r.text.starts_with('&'), "expected custom symbol: {}", r.text);
    }
}
