use lynx_theme::schema::SegmentConfig;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the current git branch name.
pub struct GitBranchSegment;

impl Segment for GitBranchSegment {
    fn name(&self) -> &'static str {
        "git_branch"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some("git_state")
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        let branch = git_state_str(ctx, "branch")?;
        if branch.is_empty() {
            return None;
        }
        let icon = config.icon.as_deref().unwrap_or(" ");
        Some(
            RenderedSegment::new(format!("{icon}{branch}"))
                .with_cache_key("git_state"),
        )
    }
}

/// Shows git status icons (staged / modified / untracked).
pub struct GitStatusSegment;

impl Segment for GitStatusSegment {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some("git_state")
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        let state = git_state_obj(ctx)?;

        let mut parts = Vec::new();

        if state.get("staged").and_then(|v| v.as_bool()).unwrap_or(false) {
            let icon = config.staged.as_ref()
                .and_then(|s| s.icon.as_deref())
                .unwrap_or("+");
            parts.push(icon.to_string());
        }
        if state.get("modified").and_then(|v| v.as_bool()).unwrap_or(false) {
            let icon = config.modified.as_ref()
                .and_then(|s| s.icon.as_deref())
                .unwrap_or("!");
            parts.push(icon.to_string());
        }
        if state.get("untracked").and_then(|v| v.as_bool()).unwrap_or(false) {
            let icon = config.untracked.as_ref()
                .and_then(|s| s.icon.as_deref())
                .unwrap_or("?");
            parts.push(icon.to_string());
        }

        if parts.is_empty() {
            return None;
        }

        Some(
            RenderedSegment::new(parts.join(""))
                .with_cache_key("git_state"),
        )
    }
}

fn git_state_obj(ctx: &RenderContext) -> Option<&serde_json::Map<String, serde_json::Value>> {
    match ctx.cache.get("git_state")? {
        serde_json::Value::Object(obj) => Some(obj),
        _ => None,
    }
}

fn git_state_str<'a>(ctx: &'a RenderContext, key: &str) -> Option<&'a str> {
    git_state_obj(ctx)?.get(key)?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::json;

    fn ctx_with_git(branch: &str, staged: bool, modified: bool, untracked: bool) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert("git_state".into(), json!({
            "branch": branch,
            "staged": staged,
            "modified": modified,
            "untracked": untracked,
        }));
        RenderContext {
            cwd: "/repo".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
        }
    }

    fn no_git_ctx() -> RenderContext {
        RenderContext {
            cwd: "/tmp".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
        }
    }

    #[test]
    fn branch_shows_when_in_repo() {
        let r = GitBranchSegment.render(&Default::default(), &ctx_with_git("main", false, false, false));
        assert!(r.is_some());
        assert!(r.unwrap().text.contains("main"));
    }

    #[test]
    fn branch_returns_none_outside_repo() {
        let r = GitBranchSegment.render(&Default::default(), &no_git_ctx());
        assert!(r.is_none());
    }

    #[test]
    fn status_hides_when_clean() {
        let r = GitStatusSegment.render(&Default::default(), &ctx_with_git("main", false, false, false));
        assert!(r.is_none());
    }

    #[test]
    fn status_shows_staged_icon() {
        let r = GitStatusSegment.render(&Default::default(), &ctx_with_git("main", true, false, false));
        assert!(r.unwrap().text.contains('+'));
    }

    #[test]
    fn status_combined_icons() {
        let r = GitStatusSegment.render(&Default::default(), &ctx_with_git("main", true, true, true));
        let text = r.unwrap().text;
        assert!(text.contains('+'));
        assert!(text.contains('!'));
        assert!(text.contains('?'));
    }
}
