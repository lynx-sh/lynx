use std::path::Path;

use serde::Deserialize;

use crate::segment::{apply_format, RenderContext, RenderedSegment, Segment};

#[derive(Deserialize, Default)]
struct DirConfig {
    max_depth: Option<u32>,
    truncate_to_repo: Option<bool>,
    /// Format template. Available vars: `$path`.
    /// Default: `"$path"`.
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

        let display = if max_depth == 0 {
            ctx.cwd.clone()
        } else {
            shorten(&ctx.cwd, max_depth, truncate_to_repo, &ctx.cache)
        };

        let text = match cfg.format.as_deref() {
            Some(tmpl) => apply_format(tmpl, &[("path", &display)]),
            None => display,
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
        RenderContext {
            cwd: cwd.to_string(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    fn cfg(s: &str) -> toml::Value {
        toml::from_str(s).unwrap()
    }

    #[test]
    fn full_path_when_max_depth_zero() {
        let r = DirSegment
            .render(&cfg("max_depth = 0\ntruncate_to_repo = false"), &ctx("/home/user/projects/lynx"))
            .unwrap();
        assert_eq!(r.text, "/home/user/projects/lynx");
    }

    #[test]
    fn truncates_at_max_depth() {
        let r = DirSegment
            .render(&cfg("max_depth = 2\ntruncate_to_repo = false"), &ctx("/a/b/c/d/e"))
            .unwrap();
        assert_eq!(r.text, "…/d/e");
    }

    #[test]
    fn no_truncation_when_short() {
        let r = DirSegment
            .render(&cfg("max_depth = 3\ntruncate_to_repo = false"), &ctx("/a/b"))
            .unwrap();
        assert_eq!(r.text, "/a/b");
    }

    #[test]
    fn default_config_renders() {
        let r = DirSegment.render(&empty_config(), &ctx("/a/b/c")).unwrap();
        assert!(!r.text.is_empty());
    }
}
