use serde::Deserialize;
use std::path::Path;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the active Terraform workspace. Hidden outside TF projects.
/// Reads .terraform/environment file — no subprocess.
///
/// TOML config:
/// ```toml
/// [segment.terraform]
/// color = { fg = "#844fba" }
/// # icon = "󱁢"
/// ```
pub struct TerraformSegment;

#[derive(Deserialize, Default)]
struct TerraformConfig {
    icon: Option<String>,
}

impl Segment for TerraformSegment {
    fn name(&self) -> &'static str {
        "terraform"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: TerraformConfig = config.clone().try_into().unwrap_or_default();
        let cwd = Path::new(&ctx.cwd);

        let env_file = cwd.join(".terraform").join("environment");
        let workspace = std::fs::read_to_string(env_file).ok()?;
        let workspace = workspace.trim();
        if workspace.is_empty() || workspace == "default" {
            return None;
        }

        let icon = cfg.icon.unwrap_or_else(|| "\u{f1bb}".to_string()); // nf-fa-tree (terraform-like)
        let text = format!("{icon} {workspace}");
        Some(RenderedSegment::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx(cwd: &str) -> RenderContext {
        RenderContext {
            cwd: cwd.to_string(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[test]
    fn hidden_outside_tf_project() {
        let dir = tempfile::tempdir().unwrap();
        let r = TerraformSegment.render(&empty_config(), &ctx(dir.path().to_str().unwrap()));
        assert!(r.is_none());
    }

    #[test]
    fn hidden_when_default_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".terraform")).unwrap();
        std::fs::write(dir.path().join(".terraform/environment"), "default").unwrap();
        let r = TerraformSegment.render(&empty_config(), &ctx(dir.path().to_str().unwrap()));
        assert!(r.is_none());
    }

    #[test]
    fn shows_non_default_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".terraform")).unwrap();
        std::fs::write(dir.path().join(".terraform/environment"), "staging").unwrap();
        let r = TerraformSegment.render(&empty_config(), &ctx(dir.path().to_str().unwrap())).unwrap();
        assert!(r.text.contains("staging"));
    }
}
