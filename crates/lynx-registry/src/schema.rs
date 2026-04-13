use serde::{Deserialize, Serialize};

/// A specific version entry in the registry for a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginVersion {
    /// Semver version string (e.g. "1.2.3").
    pub version: String,
    /// URL to the .tar.gz archive.
    pub url: String,
    /// SHA-256 hex digest of the archive. Required — fetch will refuse to
    /// install without this.
    pub checksum_sha256: String,
    /// Minimum Lynx version required to load this plugin.
    #[serde(default)]
    pub min_lynx_version: Option<String>,
}

/// Package type in the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    #[default]
    Plugin,
    Tool,
    Theme,
    Intro,
    Bundle,
    Workflow,
}

/// Install commands for tools — one field per package manager.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstallMethods {
    /// Homebrew formula name (e.g. "eza", "ripgrep")
    #[serde(default)]
    pub brew: Option<String>,
    /// APT package name
    #[serde(default)]
    pub apt: Option<String>,
    /// DNF package name
    #[serde(default)]
    pub dnf: Option<String>,
    /// Pacman package name
    #[serde(default)]
    pub pacman: Option<String>,
    /// Cargo crate name
    #[serde(default)]
    pub cargo: Option<String>,
    /// Direct download URL (for binaries or scripts)
    #[serde(default)]
    pub url: Option<String>,
}

/// A package entry in the registry index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RegistryEntry {
    /// Package name.
    pub name: String,
    /// Human-readable description (shown in search results).
    pub description: String,
    /// Author / maintainer name.
    #[serde(default)]
    pub author: String,
    /// Package type: plugin, tool, theme, intro, or bundle.
    #[serde(default)]
    pub package_type: PackageType,
    /// Category for browsing (e.g. "file-management", "search", "security").
    #[serde(default)]
    pub category: String,
    /// Supported platforms. Empty = all platforms.
    #[serde(default)]
    pub platform: Vec<String>,
    /// Install methods for tools (brew, apt, cargo, url, etc.).
    #[serde(default)]
    pub install: Option<InstallMethods>,
    /// System command this tool replaces (e.g. "ls" for eza, "cat" for bat).
    #[serde(default)]
    pub replaces: Option<String>,
    /// Whether this package integrates with Lynx theme colors.
    #[serde(default)]
    pub theme_integrated: bool,
    /// Whether this package ships bundled with Lynx (not downloaded).
    #[serde(default)]
    pub bundled: bool,
    /// List of package names included in a bundle.
    #[serde(default)]
    pub packages: Vec<String>,
    /// Latest available version (must match one entry in `versions`).
    pub latest_version: String,
    /// All available versions, newest-first.
    pub versions: Vec<PluginVersion>,
}

impl RegistryEntry {
    /// Resolve a specific version, or the latest if `version` is None.
    pub fn resolve_version(&self, version: Option<&str>) -> Option<&PluginVersion> {
        match version {
            Some(v) => self.versions.iter().find(|pv| pv.version == v),
            None => self
                .versions
                .iter()
                .find(|pv| pv.version == self.latest_version),
        }
    }

    /// Total byte size estimate — not stored in index, must come from fetch.
    pub fn version_count(&self) -> usize {
        self.versions.len()
    }

    pub fn is_plugin(&self) -> bool {
        self.package_type == PackageType::Plugin
    }
    pub fn is_tool(&self) -> bool {
        self.package_type == PackageType::Tool
    }
    pub fn is_theme(&self) -> bool {
        self.package_type == PackageType::Theme
    }
    pub fn is_intro(&self) -> bool {
        self.package_type == PackageType::Intro
    }
    pub fn is_bundle(&self) -> bool {
        self.package_type == PackageType::Bundle
    }
}

/// The full registry index, parsed from the index TOML file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RegistryIndex {
    #[serde(rename = "plugin", default)]
    pub plugins: Vec<RegistryEntry>,
}

impl RegistryIndex {
    /// Build a name→index HashMap for O(1) lookups.
    pub fn name_index(&self) -> std::collections::HashMap<&str, usize> {
        self.plugins
            .iter()
            .enumerate()
            .map(|(i, e)| (e.name.as_str(), i))
            .collect()
    }

    /// Look up a plugin by exact name.
    pub fn find(&self, name: &str) -> Option<&RegistryEntry> {
        self.plugins.iter().find(|e| e.name == name)
    }

    /// Fuzzy search: returns entries whose name or description contains `query`
    /// (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<&RegistryEntry> {
        let q = query.to_lowercase();
        self.plugins
            .iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&q) || e.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Filter entries by package type.
    pub fn search_by_type(&self, pkg_type: &PackageType) -> Vec<&RegistryEntry> {
        self.plugins
            .iter()
            .filter(|e| &e.package_type == pkg_type)
            .collect()
    }

    /// Filter entries by category (case-insensitive exact match).
    pub fn search_by_category(&self, category: &str) -> Vec<&RegistryEntry> {
        let cat = category.to_lowercase();
        self.plugins
            .iter()
            .filter(|e| e.category.to_lowercase() == cat)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_index_toml() -> &'static str {
        r#"
[[plugin]]
name = "git"
description = "Git integration for Lynx"
author = "proxikal"
latest_version = "1.0.0"

[[plugin.versions]]
version = "1.0.0"
url = "https://example.com/git-1.0.0.tar.gz"
checksum_sha256 = "abc123def456"
min_lynx_version = "0.1.0"

[[plugin.versions]]
version = "0.9.0"
url = "https://example.com/git-0.9.0.tar.gz"
checksum_sha256 = "older000hash"

[[plugin]]
name = "fzf"
description = "fzf-powered search for Lynx"
author = "proxikal"
latest_version = "0.2.0"

[[plugin.versions]]
version = "0.2.0"
url = "https://example.com/fzf-0.2.0.tar.gz"
checksum_sha256 = "fzfhash0200"

[[plugin]]
name = "kubectl"
description = "kubectl context switcher with prompt segment"
author = "proxikal"
latest_version = "0.1.0"

[[plugin.versions]]
version = "0.1.0"
url = "https://example.com/kubectl-0.1.0.tar.gz"
checksum_sha256 = "kubehash0100"
"#
    }

    #[test]
    fn parse_index_with_three_plugins() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        assert_eq!(idx.plugins.len(), 3);
    }

    #[test]
    fn find_plugin_by_name() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        let e = idx.find("git").unwrap();
        assert_eq!(e.latest_version, "1.0.0");
        assert_eq!(e.versions.len(), 2);
    }

    #[test]
    fn find_returns_none_for_unknown() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        assert!(idx.find("nonexistent").is_none());
    }

    #[test]
    fn resolve_latest_version() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        let e = idx.find("git").unwrap();
        let v = e.resolve_version(None).unwrap();
        assert_eq!(v.version, "1.0.0");
        assert_eq!(v.checksum_sha256, "abc123def456");
    }

    #[test]
    fn resolve_specific_version() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        let e = idx.find("git").unwrap();
        let v = e.resolve_version(Some("0.9.0")).unwrap();
        assert_eq!(v.version, "0.9.0");
        assert_eq!(v.checksum_sha256, "older000hash");
    }

    #[test]
    fn resolve_nonexistent_version_returns_none() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        let e = idx.find("git").unwrap();
        assert!(e.resolve_version(Some("99.0.0")).is_none());
    }

    #[test]
    fn search_fuzzy_by_name() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        let results = idx.search("kube");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "kubectl");
    }

    #[test]
    fn search_fuzzy_by_description() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        let results = idx.search("search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fzf");
    }

    #[test]
    fn search_no_results() {
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        assert!(idx.search("zzznomatch").is_empty());
    }

    #[test]
    fn checksum_field_required() {
        // A version entry without checksum_sha256 must fail to parse.
        let bad = r#"
[[plugin]]
name = "broken"
description = "bad"
author = "x"
latest_version = "1.0.0"
[[plugin.versions]]
version = "1.0.0"
url = "https://example.com/broken.tar.gz"
# checksum_sha256 intentionally missing
"#;
        let result = toml::from_str::<RegistryIndex>(bad);
        assert!(result.is_err(), "expected parse error for missing checksum");
    }

    // ── B18-P02: expanded schema tests ──────────────────────────────────────

    #[test]
    fn backward_compat_plugin_only_index_parses() {
        // Existing plugin-only index (no package_type field) must still parse.
        let idx: RegistryIndex = toml::from_str(sample_index_toml()).unwrap();
        assert_eq!(idx.plugins.len(), 3);
        // All entries default to PackageType::Plugin.
        assert!(idx.plugins.iter().all(|e| e.is_plugin()));
    }

    #[test]
    fn parse_tool_entry() {
        let toml = r#"
[[plugin]]
name = "eza"
description = "Modern ls replacement"
author = "lynx-sh"
package_type = "tool"
category = "file-management"
replaces = "ls"
theme_integrated = true
latest_version = "0.0.0"

[plugin.install]
brew = "eza"
apt = "eza"
cargo = "eza"

[[plugin.versions]]
version = "0.0.0"
url = "n/a"
checksum_sha256 = "n/a"
"#;
        let idx: RegistryIndex = toml::from_str(toml).unwrap();
        let eza = idx.find("eza").unwrap();
        assert!(eza.is_tool());
        assert_eq!(eza.category, "file-management");
        assert_eq!(eza.replaces.as_deref(), Some("ls"));
        assert!(eza.theme_integrated);
        let install = eza.install.as_ref().unwrap();
        assert_eq!(install.brew.as_deref(), Some("eza"));
        assert_eq!(install.apt.as_deref(), Some("eza"));
    }

    #[test]
    fn parse_theme_entry() {
        let toml = r#"
[[plugin]]
name = "catppuccin"
description = "Catppuccin theme for Lynx"
package_type = "theme"
category = "themes"
latest_version = "1.0.0"

[[plugin.versions]]
version = "1.0.0"
url = "https://example.com/catppuccin.toml"
checksum_sha256 = "abc"
"#;
        let idx: RegistryIndex = toml::from_str(toml).unwrap();
        let theme = idx.find("catppuccin").unwrap();
        assert!(theme.is_theme());
    }

    #[test]
    fn parse_bundle_entry() {
        let toml = r#"
[[plugin]]
name = "modern-cli"
description = "Modern CLI tools bundle"
package_type = "bundle"
category = "bundles"
packages = ["eza", "bat", "fd", "ripgrep"]
latest_version = "1.0.0"

[[plugin.versions]]
version = "1.0.0"
url = "n/a"
checksum_sha256 = "n/a"
"#;
        let idx: RegistryIndex = toml::from_str(toml).unwrap();
        let bundle = idx.find("modern-cli").unwrap();
        assert!(bundle.is_bundle());
        assert_eq!(bundle.packages, vec!["eza", "bat", "fd", "ripgrep"]);
    }

    #[test]
    fn search_by_type_filters_correctly() {
        let toml = r#"
[[plugin]]
name = "git"
description = "Git plugin"
package_type = "plugin"
latest_version = "1.0.0"
[[plugin.versions]]
version = "1.0.0"
url = "x"
checksum_sha256 = "x"

[[plugin]]
name = "eza"
description = "Modern ls"
package_type = "tool"
latest_version = "1.0.0"
[[plugin.versions]]
version = "1.0.0"
url = "x"
checksum_sha256 = "x"

[[plugin]]
name = "tokyo-night"
description = "Tokyo Night theme"
package_type = "theme"
latest_version = "1.0.0"
[[plugin.versions]]
version = "1.0.0"
url = "x"
checksum_sha256 = "x"
"#;
        let idx: RegistryIndex = toml::from_str(toml).unwrap();
        assert_eq!(idx.search_by_type(&PackageType::Plugin).len(), 1);
        assert_eq!(idx.search_by_type(&PackageType::Tool).len(), 1);
        assert_eq!(idx.search_by_type(&PackageType::Theme).len(), 1);
        assert_eq!(idx.search_by_type(&PackageType::Bundle).len(), 0);
    }

    #[test]
    fn search_by_category_case_insensitive() {
        let toml = r#"
[[plugin]]
name = "eza"
description = "Modern ls"
category = "File-Management"
latest_version = "1.0.0"
[[plugin.versions]]
version = "1.0.0"
url = "x"
checksum_sha256 = "x"
"#;
        let idx: RegistryIndex = toml::from_str(toml).unwrap();
        assert_eq!(idx.search_by_category("file-management").len(), 1);
        assert_eq!(idx.search_by_category("FILE-MANAGEMENT").len(), 1);
        assert_eq!(idx.search_by_category("search").len(), 0);
    }

    #[test]
    fn platform_field_parses() {
        let toml = r#"
[[plugin]]
name = "linpeas"
description = "Privilege escalation audit"
package_type = "tool"
platform = ["linux", "macos"]
latest_version = "1.0.0"
[[plugin.versions]]
version = "1.0.0"
url = "x"
checksum_sha256 = "x"
"#;
        let idx: RegistryIndex = toml::from_str(toml).unwrap();
        let entry = idx.find("linpeas").unwrap();
        assert_eq!(entry.platform, vec!["linux", "macos"]);
    }
}
