use std::time::Duration;

use serde::Deserialize;

use crate::segment::{apply_format, RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct GitBranchConfig {
    icon: Option<String>,
    /// Format template. Available vars: `$icon`, `$branch`.
    /// Default: `"$icon$branch"`.
    format: Option<String>,
}

#[derive(Deserialize, Default)]
struct StatusIconConfig {
    icon: Option<String>,
}

#[derive(Deserialize, Default)]
struct GitStatusConfig {
    staged: Option<StatusIconConfig>,
    modified: Option<StatusIconConfig>,
    untracked: Option<StatusIconConfig>,
    /// Format template. Available vars: `$staged`, `$modified`, `$untracked`.
    /// Default: `"$staged$modified$untracked"` (concatenated, each empty when clean).
    format: Option<String>,
}

/// Shows the current git branch name.
pub struct GitBranchSegment;

impl Segment for GitBranchSegment {
    fn name(&self) -> &'static str {
        "git_branch"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GIT_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GitBranchConfig = config.clone().try_into().unwrap_or_default();
        let branch = git_branch(ctx)?;
        if branch.is_empty() {
            return None;
        }
        let icon = cfg.icon.as_deref().unwrap_or(" ");
        let text = match cfg.format.as_deref() {
            Some(tmpl) => apply_format(tmpl, &[("icon", icon), ("branch", &branch)]),
            None => format!("{icon}{branch}"),
        };
        Some(RenderedSegment::new(text).with_cache_key("git_branch"))
    }
}

/// Shows git status icons (staged / modified / untracked).
pub struct GitStatusSegment;

impl Segment for GitStatusSegment {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn cache_key(&self) -> Option<&'static str> {
        Some(crate::cache_keys::GIT_STATE)
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: GitStatusConfig = config.clone().try_into().unwrap_or_default();
        let state = git_state_obj(ctx)?;

        let mut parts = Vec::new();

        if state.get("staged").and_then(|v| v.as_bool()).unwrap_or(false) {
            let icon = cfg.staged.as_ref().and_then(|s| s.icon.as_deref()).unwrap_or("+");
            parts.push(icon.to_string());
        }
        if state.get("modified").and_then(|v| v.as_bool()).unwrap_or(false) {
            let icon = cfg.modified.as_ref().and_then(|s| s.icon.as_deref()).unwrap_or("!");
            parts.push(icon.to_string());
        }
        if state.get("untracked").and_then(|v| v.as_bool()).unwrap_or(false) {
            let icon = cfg.untracked.as_ref().and_then(|s| s.icon.as_deref()).unwrap_or("?");
            parts.push(icon.to_string());
        }

        if parts.is_empty() {
            return None;
        }

        let text = match cfg.format.as_deref() {
            Some(tmpl) => {
                let staged_icon = cfg.staged.as_ref().and_then(|s| s.icon.as_deref()).unwrap_or("+");
                let modified_icon = cfg.modified.as_ref().and_then(|s| s.icon.as_deref()).unwrap_or("!");
                let untracked_icon = cfg.untracked.as_ref().and_then(|s| s.icon.as_deref()).unwrap_or("?");
                let state = git_state_obj(ctx)?;
                let staged_val = if state.get("staged").and_then(|v| v.as_bool()).unwrap_or(false) { staged_icon } else { "" };
                let modified_val = if state.get("modified").and_then(|v| v.as_bool()).unwrap_or(false) { modified_icon } else { "" };
                let untracked_val = if state.get("untracked").and_then(|v| v.as_bool()).unwrap_or(false) { untracked_icon } else { "" };
                apply_format(tmpl, &[("staged", staged_val), ("modified", modified_val), ("untracked", untracked_val)])
            }
            None => parts.join(""),
        };

        Some(RenderedSegment::new(text).with_cache_key(crate::cache_keys::GIT_STATE))
    }
}

/// Resolve the git branch from cache, falling back to a direct `git` call.
///
/// Priority:
/// 1. `git_state` cache (populated by the git plugin via LYNX_CACHE_GIT_STATE)
/// 2. Direct `git -C <cwd> symbolic-ref --short HEAD` with a 200ms timeout
///
/// The fallback means the branch segment works even without the git plugin loaded.
fn git_branch(ctx: &RenderContext) -> Option<String> {
    if let Some(branch) = git_state_str(ctx, "branch") {
        if !branch.is_empty() {
            return Some(branch.to_string());
        }
    }
    git_branch_from_subprocess(&ctx.cwd)
}

/// Call `git -C <dir> symbolic-ref --short HEAD` with a 200ms wall-clock timeout.
fn git_branch_from_subprocess(dir: &str) -> Option<String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("git")
        .args(["-C", dir, "symbolic-ref", "--short", "HEAD"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let timeout = Duration::from_millis(200);
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(10);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    let output = child.wait_with_output().ok()?;
                    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    return if branch.is_empty() { None } else { Some(branch) };
                } else {
                    return None;
                }
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    return None;
                }
                std::thread::sleep(poll_interval);
            }
            Err(_) => return None,
        }
    }
}

fn git_state_obj(ctx: &RenderContext) -> Option<&serde_json::Map<String, serde_json::Value>> {
    match ctx.cache.get(crate::cache_keys::GIT_STATE)? {
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
    use crate::segment::empty_config;
    use serde_json::json;
    use std::collections::HashMap;

    fn ctx_with_git(branch: &str, staged: bool, modified: bool, untracked: bool) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            crate::cache_keys::GIT_STATE.into(),
            json!({
                "branch": branch,
                "staged": staged,
                "modified": modified,
                "untracked": untracked,
            }),
        );
        RenderContext {
            cwd: "/repo".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
            env: HashMap::new(),
        }
    }

    fn no_git_ctx() -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[test]
    fn branch_shows_when_in_repo() {
        let r = GitBranchSegment.render(&empty_config(), &ctx_with_git("main", false, false, false));
        assert!(r.is_some());
        assert!(r.unwrap().text.contains("main"));
    }

    #[test]
    fn branch_returns_none_outside_repo() {
        let r = GitBranchSegment.render(&empty_config(), &no_git_ctx());
        assert!(r.is_none());
    }

    #[test]
    fn branch_fallback_works_in_real_repo() {
        let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
            .map(|d| {
                std::path::PathBuf::from(d)
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| ".".into())
            })
            .unwrap_or_else(|_| ".".into());

        let branch = git_branch_from_subprocess(&workspace_root);
        assert!(
            branch.is_some(),
            "expected a branch from workspace git root at {workspace_root}"
        );
    }

    #[test]
    fn status_hides_when_clean() {
        let r = GitStatusSegment.render(&empty_config(), &ctx_with_git("main", false, false, false));
        assert!(r.is_none());
    }

    #[test]
    fn status_shows_staged_icon() {
        let r = GitStatusSegment.render(&empty_config(), &ctx_with_git("main", true, false, false));
        assert!(r.unwrap().text.contains('+'));
    }

    #[test]
    fn status_combined_icons() {
        let r = GitStatusSegment.render(&empty_config(), &ctx_with_git("main", true, true, true));
        let text = r.unwrap().text;
        assert!(text.contains('+'));
        assert!(text.contains('!'));
        assert!(text.contains('?'));
    }

    #[test]
    fn branch_custom_format_changes_layout() {
        let cfg: toml::Value = toml::from_str(r#"format = "[$branch] ""#).unwrap();
        let r = GitBranchSegment.render(&cfg, &ctx_with_git("main", false, false, false)).unwrap();
        assert_eq!(r.text, "[main] ");
    }

    #[test]
    fn branch_default_format_matches_current_output() {
        let r_default = GitBranchSegment.render(&empty_config(), &ctx_with_git("main", false, false, false)).unwrap();
        let cfg: toml::Value = toml::from_str(r#"format = "$icon$branch""#).unwrap();
        let r_format = GitBranchSegment.render(&cfg, &ctx_with_git("main", false, false, false)).unwrap();
        assert_eq!(r_default.text, r_format.text);
    }

    #[test]
    fn status_custom_format_with_brackets() {
        let cfg: toml::Value = toml::from_str(r#"format = "[$staged$modified$untracked]""#).unwrap();
        let r = GitStatusSegment.render(&cfg, &ctx_with_git("main", true, false, true)).unwrap();
        assert_eq!(r.text, "[+?]");
    }

    #[test]
    fn status_unknown_format_var_is_empty() {
        let cfg: toml::Value = toml::from_str(r#"format = "$staged$unknown""#).unwrap();
        let r = GitStatusSegment.render(&cfg, &ctx_with_git("main", true, false, false)).unwrap();
        assert_eq!(r.text, "+");
    }
}
