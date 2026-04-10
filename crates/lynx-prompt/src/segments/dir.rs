use std::path::Path;

use lynx_theme::schema::SegmentConfig;

use crate::segment::{RenderContext, RenderedSegment, Segment};

pub struct DirSegment;

impl Segment for DirSegment {
    fn name(&self) -> &'static str {
        "dir"
    }

    fn render(&self, config: &SegmentConfig, ctx: &RenderContext) -> Option<RenderedSegment> {
        let max_depth = config.max_depth.unwrap_or(3);
        let truncate_to_repo = config.truncate_to_repo.unwrap_or(true);

        let display = if max_depth == 0 {
            // max_depth == 0 means show full path
            ctx.cwd.clone()
        } else {
            shorten(&ctx.cwd, max_depth, truncate_to_repo, &ctx.cache)
        };

        Some(RenderedSegment::new(display))
    }
}

fn shorten(
    cwd: &str,
    max_depth: u32,
    truncate_to_repo: bool,
    cache: &std::collections::HashMap<String, serde_json::Value>,
) -> String {
    // If truncate_to_repo, find the repo root from cache and show relative path.
    if truncate_to_repo {
        if let Some(serde_json::Value::Object(obj)) = cache.get("git_state") {
            if let Some(serde_json::Value::String(root)) = obj.get("repo_root") {
                if let Some(rel) = cwd.strip_prefix(root.as_str()) {
                    let rel = rel.trim_start_matches('/');
                    let repo_name = Path::new(root).file_name()
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

    // No repo root — shorten from the tail.
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

    fn ctx(cwd: &str) -> RenderContext {
        RenderContext {
            cwd: cwd.to_string(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
        }
    }

    #[test]
    fn full_path_when_max_depth_zero() {
        let seg = DirSegment;
        let cfg = SegmentConfig { max_depth: Some(0), truncate_to_repo: Some(false), ..Default::default() };
        let r = seg.render(&cfg, &ctx("/home/user/projects/lynx")).unwrap();
        assert_eq!(r.text, "/home/user/projects/lynx");
    }

    #[test]
    fn truncates_at_max_depth() {
        let seg = DirSegment;
        let cfg = SegmentConfig { max_depth: Some(2), truncate_to_repo: Some(false), ..Default::default() };
        let r = seg.render(&cfg, &ctx("/a/b/c/d/e")).unwrap();
        assert_eq!(r.text, "…/d/e");
    }

    #[test]
    fn no_truncation_when_short() {
        let seg = DirSegment;
        let cfg = SegmentConfig { max_depth: Some(3), truncate_to_repo: Some(false), ..Default::default() };
        let r = seg.render(&cfg, &ctx("/a/b")).unwrap();
        assert_eq!(r.text, "/a/b");
    }
}
