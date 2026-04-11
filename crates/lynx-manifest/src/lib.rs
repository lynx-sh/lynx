pub mod schema;
pub mod validator;

use lynx_core::error::Result;
use schema::PluginManifest;

/// Parse a plugin.toml from a TOML string.
pub fn parse(toml_str: &str) -> Result<PluginManifest> {
    toml::from_str(toml_str).map_err(|e| lynx_core::error::LynxError::Manifest(e.to_string()))
}

/// Parse and validate a plugin.toml string in one step.
pub fn parse_and_validate(toml_str: &str) -> Result<PluginManifest> {
    let manifest = parse(toml_str)?;
    validator::validate(&manifest)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIT_PLUGIN_TOML: &str = r#"
[plugin]
name        = "git"
version     = "0.1.0"
description = "Git integration"
authors     = ["proxikal"]

[load]
lazy  = false
hooks = ["chpwd"]

[deps]
binaries = ["git"]
plugins  = []

[exports]
functions = ["git_branch", "git_dirty"]
aliases   = ["gst", "gco"]

[contexts]
disabled_in = ["agent", "minimal"]
"#;

    #[test]
    fn parse_git_plugin_toml() {
        let m = parse(GIT_PLUGIN_TOML).expect("should parse");
        assert_eq!(m.plugin.name, "git");
        assert_eq!(m.load.hooks, vec!["chpwd"]);
        assert_eq!(m.deps.binaries, vec!["git"]);
        assert!(m.exports.functions.contains(&"git_branch".to_string()));
        assert!(m.contexts.disabled_in.contains(&"agent".to_string()));
    }

    #[test]
    fn schema_version_defaults_to_current() {
        let m = parse(GIT_PLUGIN_TOML).unwrap();
        assert_eq!(m.schema_version, schema::CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn state_gather_is_optional_and_defaults_to_none() {
        let m = parse(GIT_PLUGIN_TOML).unwrap();
        assert!(m.state.gather.is_none());
    }

    #[test]
    fn state_gather_parses_when_set() {
        let toml = r#"
[plugin]
name    = "myplugin"
version = "0.1.0"

[state]
gather = "myplugin state"
"#;
        let m = parse(toml).unwrap();
        assert_eq!(m.state.gather.as_deref(), Some("myplugin state"));
    }

    #[test]
    fn plugin_toml_without_state_section_still_parses() {
        // Backwards compat: all existing plugin.toml files have no [state] section.
        let toml = r#"
[plugin]
name    = "legacy"
version = "0.1.0"

[exports]
functions = ["my_fn"]
aliases   = []
"#;
        let m = parse(toml).unwrap();
        assert!(m.state.gather.is_none());
    }
}
