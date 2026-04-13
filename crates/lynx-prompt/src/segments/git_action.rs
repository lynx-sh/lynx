use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct GitActionConfig {
    /// Override labels per action type. Keys: merge, rebase, cherry-pick, bisect.
    label: Option<std::collections::HashMap<String, String>>,
}

pub struct GitActionSegment;

use super::git_common::git_state_obj;

impl Segment for GitActionSegment {
    fn name(&self) -> &'static str {
        "git_action"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GIT_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GitActionConfig = config.clone().try_into().unwrap_or_default();
        let obj = git_state_obj(ctx)?;
        let action = obj.get("action")?.as_str()?;
        if action.is_empty() {
            return None;
        }
        let text = cfg
            .label
            .as_ref()
            .and_then(|m| m.get(action).cloned())
            .unwrap_or_else(|| action.to_uppercase());
        Some(RenderedSegment::new(text).with_cache_key("git_action"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_action(action: Option<&str>) -> RenderContext {
        let mut cache = HashMap::new();
        let val = match action {
            Some(a) => serde_json::json!({ "branch": "main", "action": a }),
            None => serde_json::json!({ "branch": "main", "action": null }),
        };
        cache.insert(crate::cache_keys::GIT_STATE.into(), val);
        RenderContext {
            cwd: "/repo".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
            env: HashMap::new(),
        }
    }

    #[test]
    fn hidden_when_no_action() {
        let ctx = ctx_with_action(None);
        let r = GitActionSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_no_cache() {
        let ctx = RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        };
        let r = GitActionSegment.render(&empty_config(), &ctx);
        assert!(r.is_none());
    }

    #[test]
    fn shows_merge_action() {
        let ctx = ctx_with_action(Some("merge"));
        let r = GitActionSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "MERGE");
    }

    #[test]
    fn shows_rebase_action() {
        let ctx = ctx_with_action(Some("rebase"));
        let r = GitActionSegment.render(&empty_config(), &ctx).unwrap();
        assert_eq!(r.text, "REBASE");
    }

    #[test]
    fn custom_label_overrides_default() {
        let cfg: toml::Value = toml::from_str(r#"[label]
merge = "⚡MERGING"
"#).unwrap();
        let ctx = ctx_with_action(Some("merge"));
        let r = GitActionSegment.render(&cfg, &ctx).unwrap();
        assert_eq!(r.text, "⚡MERGING");
    }
}
