use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use lynx_core::{
    error::{LynxError, Result},
    redact::looks_like_secret_value,
};

/// A named profile stored at `~/.config/lynx/profiles/<name>.toml`.
///
/// Profiles select a plugin set, theme, and env vars to apply to the shell
/// session. They are **orthogonal to context**: context gates what *can* load;
/// profiles select what *should* load from the available set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Profile name — must match the file stem.
    pub name: String,

    /// Optional base profile to inherit from (single-level only, no chains).
    #[serde(default)]
    pub extends: Option<String>,

    /// Plugins enabled by this profile. Merges with parent (child wins on conflict).
    #[serde(default)]
    pub plugins: Vec<String>,

    /// Theme name. Child overrides parent.
    #[serde(default)]
    pub theme: Option<String>,

    /// Suggested context — not enforced, just informational.
    #[serde(default)]
    pub context_override: Option<String>,

    /// Env vars to export. Secret-shaped values produce a warning at parse time.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Shell aliases. Loaded only in interactive context (context filter applies).
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

/// A parse warning — returned alongside a valid Profile, not fatal.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileWarning {
    pub key: String,
    pub message: String,
}

/// Parse a profile TOML string. Returns the profile and any warnings.
pub fn parse(toml_str: &str) -> Result<(Profile, Vec<ProfileWarning>)> {
    let profile: Profile = toml::from_str(toml_str)
        .map_err(|e| LynxError::Config(format!("profile parse error: {e}")))?;
    let warnings = check_warnings(&profile);
    Ok((profile, warnings))
}

/// Load a profile file by name from the profiles directory.
/// Does NOT resolve extends — call `resolve` for the merged result.
pub fn load(name: &str) -> Result<(Profile, Vec<ProfileWarning>)> {
    load_from(&profiles_dir().join(format!("{name}.toml")))
}

/// Load from an explicit path.
pub fn load_from(path: &Path) -> Result<(Profile, Vec<ProfileWarning>)> {
    let content = std::fs::read_to_string(path).map_err(|e| LynxError::io(e, path))?;
    parse(&content)
}

/// Resolve a profile by name: loads it, follows extends (single level), and
/// returns the merged profile plus all warnings from both layers.
pub fn resolve(name: &str) -> Result<(Profile, Vec<ProfileWarning>)> {
    resolve_from(&profiles_dir(), name)
}

pub fn resolve_from(dir: &Path, name: &str) -> Result<(Profile, Vec<ProfileWarning>)> {
    let path = dir.join(format!("{name}.toml"));
    let content = std::fs::read_to_string(&path).map_err(|e| LynxError::io(e, &path))?;
    let (mut child, mut warnings) = parse(&content)?;

    if let Some(ref parent_name) = child.extends.clone() {
        // Guard against self-extension and deep chains.
        if parent_name == name {
            return Err(LynxError::Config(format!(
                "profile '{name}' cannot extend itself"
            )));
        }
        let parent_path = dir.join(format!("{parent_name}.toml"));
        if !parent_path.exists() {
            return Err(LynxError::Config(format!(
                "profile '{name}' extends '{parent_name}' which does not exist"
            )));
        }
        let (parent, parent_warnings) = parse(
            &std::fs::read_to_string(&parent_path).map_err(|e| LynxError::io(e, &parent_path))?,
        )?;
        warnings.extend(parent_warnings);
        child = merge(parent, child);
    }

    Ok((child, warnings))
}

/// List all profile names available in the profiles directory.
pub fn list_names() -> Result<Vec<String>> {
    list_names_in(&profiles_dir())
}

pub fn list_names_in(dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(LynxError::IoRaw)? {
        let entry = entry.map_err(LynxError::IoRaw)?;
        let path = entry.path();
        if path.extension().map(|e| e == "toml").unwrap_or(false) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Profiles directory: `~/.config/lynx/profiles/`.
pub fn profiles_dir() -> PathBuf {
    lynx_core::paths::profiles_dir()
}

/// Merge parent + child: child fields override parent, plugin lists are union-deduped.
fn merge(parent: Profile, child: Profile) -> Profile {
    // Union plugins: start with parent, append child-only plugins.
    let mut plugins = parent.plugins.clone();
    for p in &child.plugins {
        if !plugins.contains(p) {
            plugins.push(p.clone());
        }
    }

    // Merge env: child overrides parent keys.
    let mut env = parent.env.clone();
    env.extend(child.env.clone());

    // Merge aliases: child overrides parent keys.
    let mut aliases = parent.aliases.clone();
    aliases.extend(child.aliases.clone());

    Profile {
        name: child.name,
        extends: child.extends,
        plugins,
        theme: child.theme.or(parent.theme),
        context_override: child.context_override.or(parent.context_override),
        env,
        aliases,
    }
}

fn check_warnings(profile: &Profile) -> Vec<ProfileWarning> {
    let mut warnings = Vec::new();
    for (key, value) in &profile.env {
        if looks_like_secret_value(key, value) {
            warnings.push(ProfileWarning {
                key: key.clone(),
                message: format!(
                    "env key '{key}' looks like a secret — consider using a secrets manager instead of storing in profile"
                ),
            });
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_toml(extra: &str) -> String {
        format!(
            r#"
name = "test"
plugins = ["git", "fzf"]
theme = "default"
{extra}
"#
        )
    }

    #[test]
    fn parse_basic_profile() {
        let (p, warns) = parse(&profile_toml("")).unwrap();
        assert_eq!(p.name, "test");
        assert_eq!(p.plugins, ["git", "fzf"]);
        assert_eq!(p.theme, Some("default".into()));
        assert!(warns.is_empty());
    }

    #[test]
    fn secret_env_value_produces_warning() {
        let toml = profile_toml(
            r#"[env]
GITHUB_TOKEN = "ghp_abc123secret"
"#,
        );
        let (_, warns) = parse(&toml).unwrap();
        assert!(!warns.is_empty());
        assert!(warns[0].key == "GITHUB_TOKEN");
    }

    #[test]
    fn non_secret_env_no_warning() {
        let toml = profile_toml(
            r#"[env]
EDITOR = "nvim"
"#,
        );
        let (_, warns) = parse(&toml).unwrap();
        assert!(warns.is_empty());
    }

    #[test]
    fn resolve_merges_parent_and_child() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();

        std::fs::write(
            d.join("base.toml"),
            r#"
name = "base"
plugins = ["git"]
theme = "default"
[env]
FOO = "bar"
"#,
        )
        .unwrap();

        std::fs::write(
            d.join("work.toml"),
            r#"
name = "work"
extends = "base"
plugins = ["kubectl"]
theme = "nord"
[env]
BAR = "baz"
"#,
        )
        .unwrap();

        let (resolved, warns) = resolve_from(d, "work").unwrap();
        assert!(warns.is_empty());
        assert!(resolved.plugins.contains(&"git".to_string()));
        assert!(resolved.plugins.contains(&"kubectl".to_string()));
        assert_eq!(resolved.theme, Some("nord".into())); // child wins
        assert_eq!(resolved.env["FOO"], "bar"); // from parent
        assert_eq!(resolved.env["BAR"], "baz"); // from child
    }

    #[test]
    fn invalid_extends_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("child.toml"),
            r#"
name = "child"
extends = "nonexistent"
plugins = []
"#,
        )
        .unwrap();
        let result = resolve_from(dir.path(), "child");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nonexistent"));
    }

    #[test]
    fn self_extends_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("loop.toml"),
            r#"
name = "loop"
extends = "loop"
plugins = []
"#,
        )
        .unwrap();
        let result = resolve_from(dir.path(), "loop");
        assert!(result.is_err());
    }

    #[test]
    fn list_names_returns_sorted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("work.toml"), "name=\"work\"\nplugins=[]").unwrap();
        std::fs::write(
            dir.path().join("default.toml"),
            "name=\"default\"\nplugins=[]",
        )
        .unwrap();
        let names = list_names_in(dir.path()).unwrap();
        assert_eq!(names, ["default", "work"]);
    }
}
