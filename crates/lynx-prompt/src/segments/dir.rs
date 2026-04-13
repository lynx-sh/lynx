use std::path::Path;

use serde::Deserialize;

use crate::segment::{apply_format, RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct DirConfig {
    icon: Option<String>,
    max_depth: Option<u32>,
    truncate_to_repo: Option<bool>,
    /// Replace the home directory prefix with `~`. Default: `true`.
    tilde_home: Option<bool>,
    /// Format template. Available vars: `$icon`, `$path`.
    /// Default: `"$icon$path"`.
    format: Option<String>,
}

pub struct DirSegment;

impl Segment for DirSegment {
    fn name(&self) -> &'static str {
        "dir"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: DirConfig = config.clone().try_into().unwrap_or_default();
        let max_depth = cfg.max_depth.unwrap_or(3);
        let truncate_to_repo = cfg.truncate_to_repo.unwrap_or(true);
        let tilde_home = cfg.tilde_home.unwrap_or(true);

        let home = ctx.env.get("HOME").map(|s| s.as_str()).unwrap_or("");
        let effective_cwd = if tilde_home && !home.is_empty() {
            if ctx.cwd == home {
                "~".to_string()
            } else if let Some(rest) = ctx.cwd.strip_prefix(home) {
                format!("~{rest}")
            } else {
                ctx.cwd.clone()
            }
        } else {
            ctx.cwd.clone()
        };

        let display = if max_depth == 0 {
            effective_cwd
        } else {
            shorten(&effective_cwd, max_depth, truncate_to_repo, &ctx.cache)
        };

        let icon = cfg.icon.as_deref().unwrap_or("");
        let text = match cfg.format.as_deref() {
            Some(tmpl) => apply_format(tmpl, &[("icon", icon), ("path", &display)]),
            None if icon.is_empty() => display,
            None => format!("{icon}{display}"),
        };
        Some(RenderedSegment::new(text))
    }
}

fn shorten(
    cwd: &str,
    max_depth: u32,
    truncate_to_repo: bool,
    cache: &std::collections::HashMap<String, serde_json::Value>,
) -> String {
    if truncate_to_repo {
        if let Some(serde_json::Value::Object(obj)) = cache.get(crate::cache_keys::GIT_STATE) {
            if let Some(serde_json::Value::String(root)) = obj.get("repo_root") {
                if let Some(rel) = cwd.strip_prefix(root.as_str()) {
                    let rel = rel.trim_start_matches('/');
                    let repo_name = Path::new(root)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("repo");
                    if rel.is_empty() {
                        return repo_name.to_string();
                    }
                    let shortened = shorten_components(rel, max_depth);
                    return format!("{repo_name}/{shortened}");
                }
            }
        }
    }

    shorten_components(cwd, max_depth)
}

fn shorten_components(path: &str, max_depth: u32) -> String {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    let depth = max_depth as usize;
    if parts.len() <= depth {
        return parts.join("/");
    }
    let tail: Vec<&str> = parts[parts.len() - depth..].to_vec();
    format!("…/{}", tail.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::segment::empty_config;

    fn ctx(cwd: &str) -> RenderContext {
        ctx_with_home(cwd, "")
    }

    fn ctx_with_home(cwd: &str, home: &str) -> RenderContext {
        let mut env = HashMap::new();
        if !home.is_empty() {
            env.insert("HOME".to_string(), home.to_string());
        }
        RenderContext {
            cwd: cwd.to_string(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env,
        }
    }

    fn cfg(s: &str) -> toml::Value {
        toml::from_str(s).unwrap()
    }

    #[test]
    fn full_path_when_max_depth_zero() {
        let r = DirSegment
            .render(
                &cfg("max_depth = 0\ntruncate_to_repo = false"),
                &ctx("/home/user/projects/lynx"),
            )
            .unwrap();
        assert_eq!(r.text, "/home/user/projects/lynx");
    }

    #[test]
    fn truncates_at_max_depth() {
        let r = DirSegment
            .render(
                &cfg("max_depth = 2\ntruncate_to_repo = false"),
                &ctx("/a/b/c/d/e"),
            )
            .unwrap();
        assert_eq!(r.text, "…/d/e");
    }

    #[test]
    fn no_truncation_when_short() {
        let r = DirSegment
            .render(
                &cfg("max_depth = 3\ntruncate_to_repo = false"),
                &ctx("/a/b"),
            )
            .unwrap();
        assert_eq!(r.text, "/a/b");
    }

    #[test]
    fn default_config_renders() {
        let r = DirSegment.render(&empty_config(), &ctx("/a/b/c")).unwrap();
        assert!(!r.text.is_empty());
    }

    #[test]
    fn tilde_substitution_home_dir() {
        let r = DirSegment
            .render(
                &cfg("max_depth = 0\ntruncate_to_repo = false"),
                &ctx_with_home("/home/user", "/home/user"),
            )
            .unwrap();
        assert_eq!(r.text, "~");
    }

    #[test]
    fn tilde_substitution_subdir() {
        let r = DirSegment
            .render(
                &cfg("max_depth = 0\ntruncate_to_repo = false"),
                &ctx_with_home("/home/user/dev/projects", "/home/user"),
            )
            .unwrap();
        assert_eq!(r.text, "~/dev/projects");
    }

    #[test]
    fn tilde_substitution_with_truncation() {
        let r = DirSegment
            .render(
                &cfg("max_depth = 2\ntruncate_to_repo = false"),
                &ctx_with_home("/home/user/a/b/c/d", "/home/user"),
            )
            .unwrap();
        assert_eq!(r.text, "…/c/d");
    }

    #[test]
    fn tilde_substitution_disabled() {
        let r = DirSegment
            .render(
                &cfg("max_depth = 0\ntruncate_to_repo = false\ntilde_home = false"),
                &ctx_with_home("/home/user", "/home/user"),
            )
            .unwrap();
        assert_eq!(r.text, "/home/user");
    }

    #[test]
    fn no_tilde_when_not_under_home() {
        let r = DirSegment
            .render(
                &cfg("max_depth = 0\ntruncate_to_repo = false"),
                &ctx_with_home("/tmp/work", "/home/user"),
            )
            .unwrap();
        assert_eq!(r.text, "/tmp/work");
    }
}
