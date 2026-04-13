use std::path::PathBuf;

use anyhow::{Context as _, Result};

use crate::schema::Intro;

/// Built-in preset intros bundled into the binary.
const BUILTIN_INTROS: &[(&str, &str)] = &[
    ("hacker",    include_str!("../intros/hacker.toml")),
    ("minimal",   include_str!("../intros/minimal.toml")),
    ("neofetch",  include_str!("../intros/neofetch.toml")),
    ("welcome",   include_str!("../intros/welcome.toml")),
    ("poweruser", include_str!("../intros/poweruser.toml")),
];

/// An entry in the merged intro list (built-in + user).
#[derive(Debug, Clone)]
pub struct IntroEntry {
    pub slug: String,
    pub name: String,
    pub is_builtin: bool,
}

/// List all built-in intro slugs.
pub fn list_builtin() -> Vec<&'static str> {
    BUILTIN_INTROS.iter().map(|(slug, _)| *slug).collect()
}

/// Load a built-in intro by slug. Returns `None` if the slug is not a built-in.
pub fn load_builtin(slug: &str) -> Option<Intro> {
    BUILTIN_INTROS
        .iter()
        .find(|(s, _)| *s == slug)
        .and_then(|(_, content)| toml::from_str(content).ok())
}

/// The user intro directory: `<lynx_dir>/intros/`.
pub fn user_intro_dir() -> PathBuf {
    lynx_core::paths::lynx_dir().join("intros")
}

/// Load a user intro from `<lynx_dir>/intros/<slug>.toml`.
pub fn load_user(slug: &str) -> Result<Intro> {
    // Slug must be a plain identifier — no path traversal.
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") {
        return Err(lynx_core::error::LynxError::Theme(format!("invalid intro slug '{slug}': must not contain path separators")).into());
    }
    let path = user_intro_dir().join(format!("{slug}.toml"));
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read user intro '{slug}' at {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("failed to parse user intro '{slug}'"))
}

/// Load an intro by slug — tries user dir first, then built-ins.
pub fn load(slug: &str) -> Result<Intro> {
    // Slug must be a plain identifier.
    if slug.contains('/') || slug.contains('\\') || slug.contains("..") {
        return Err(lynx_core::error::LynxError::Theme(format!("invalid intro slug '{slug}': must not contain path separators")).into());
    }

    // User intro overrides built-in.
    let user_path = user_intro_dir().join(format!("{slug}.toml"));
    if user_path.exists() {
        return load_user(slug);
    }

    load_builtin(slug)
        .ok_or_else(|| anyhow::Error::from(lynx_core::error::LynxError::NotFound {
            item_type: "Intro".into(),
            name: slug.to_string(),
            hint: "run `lx intro list` to see available intros".into(),
        }))
}

/// List all available intros: user intros first, then built-ins not shadowed by a user intro.
pub fn list_all() -> Vec<IntroEntry> {
    let mut entries: Vec<IntroEntry> = Vec::new();
    let user_dir = user_intro_dir();

    // User intros from disk.
    if let Ok(read_dir) = std::fs::read_dir(&user_dir) {
        let mut user_slugs: Vec<String> = read_dir
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()?.to_str()? == "toml" {
                    path.file_stem()?.to_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();
        user_slugs.sort();
        for slug in user_slugs {
            let name = load_user(&slug)
                .map(|i| i.meta.name.clone())
                .unwrap_or_else(|_| slug.clone());
            entries.push(IntroEntry { slug, name, is_builtin: false });
        }
    }

    // Built-ins not already shadowed by a user intro.
    let user_slugs: std::collections::HashSet<_> = entries.iter().map(|e| e.slug.clone()).collect();
    for (slug, content) in BUILTIN_INTROS {
        if user_slugs.contains(*slug) {
            continue;
        }
        let name = toml::from_str::<Intro>(content)
            .map(|i| i.meta.name.clone())
            .unwrap_or_else(|_| slug.to_string());
        entries.push(IntroEntry {
            slug: slug.to_string(),
            name,
            is_builtin: true,
        });
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_builtin_returns_five_slugs() {
        let slugs = list_builtin();
        assert_eq!(slugs.len(), 5);
        assert!(slugs.contains(&"hacker"));
        assert!(slugs.contains(&"minimal"));
        assert!(slugs.contains(&"neofetch"));
        assert!(slugs.contains(&"welcome"));
        assert!(slugs.contains(&"poweruser"));
    }

    #[test]
    fn each_builtin_loads_without_error() {
        for slug in list_builtin() {
            let result = load_builtin(slug);
            assert!(result.is_some(), "builtin '{}' failed to parse", slug);
            let intro = result.unwrap();
            assert_eq!(intro.meta.name, slug, "meta.name mismatch for '{}'", slug);
        }
    }

    #[test]
    fn load_nonexistent_returns_error() {
        let result = load("this_does_not_exist");
        assert!(result.is_err());
    }

    #[test]
    fn load_builtin_via_load() {
        let intro = load("minimal").unwrap();
        assert_eq!(intro.meta.name, "minimal");
        assert!(!intro.blocks.is_empty());
    }

    #[test]
    fn list_all_no_user_dir_returns_builtins() {
        // Without a user dir, list_all() returns exactly the 5 built-ins.
        // (In CI / test env, user_intro_dir() likely does not exist.)
        let all = list_all();
        // We can't assert exact count because a dev machine may have a real user dir,
        // but we can assert all built-in slugs are present.
        let slugs: Vec<&str> = all.iter().map(|e| e.slug.as_str()).collect();
        for slug in list_builtin() {
            assert!(slugs.contains(&slug), "missing builtin '{}' in list_all()", slug);
        }
    }

    #[test]
    fn path_traversal_slug_rejected() {
        assert!(load("../etc/passwd").is_err());
        assert!(load_user("../evil").is_err());
    }

}
