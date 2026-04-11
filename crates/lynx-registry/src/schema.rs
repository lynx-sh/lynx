use serde::{Deserialize, Serialize};

/// A specific version entry in the registry for a plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// A plugin entry in the registry index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Plugin name — must match the plugin.toml `[plugin].name` field.
    pub name: String,
    /// Human-readable description (shown in search results).
    pub description: String,
    /// Author / maintainer name.
    #[serde(default)]
    pub author: String,
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
}

/// The full registry index, parsed from the index TOML file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RegistryIndex {
    #[serde(rename = "plugin", default)]
    pub plugins: Vec<RegistryEntry>,
}

impl RegistryIndex {
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
}

/// A single entry in lynx.lock — pins an installed plugin to an exact version
/// and checksum so future installs are reproducible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockEntry {
    /// Plugin name.
    pub name: String,
    /// Exact version installed.
    pub version: String,
    /// SHA-256 hex digest of the installed archive (verified at install time).
    pub checksum_sha256: String,
    /// SHA-256 hex digest of the installed plugin directory contents.
    /// Used by `lx plugin checksum <name>` for post-install tamper checks.
    #[serde(default)]
    pub installed_checksum_sha256: Option<String>,
    /// Source URL the archive was downloaded from.
    pub url: String,
    /// Install method: "registry" or "local".
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "registry".to_string()
}

/// The full lynx.lock file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LockFile {
    #[serde(rename = "locked", default)]
    pub entries: Vec<LockEntry>,
}

impl LockFile {
    /// Find a locked entry by plugin name.
    pub fn find(&self, name: &str) -> Option<&LockEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Upsert: replace an existing entry or append a new one.
    pub fn upsert(&mut self, entry: LockEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.name == entry.name) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Remove an entry by name. Returns true if it was present.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.name != name);
        self.entries.len() < before
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

    #[test]
    fn lockfile_upsert_and_find() {
        let mut lock = LockFile::default();
        lock.upsert(LockEntry {
            name: "git".into(),
            version: "1.0.0".into(),
            checksum_sha256: "abc".into(),
            installed_checksum_sha256: Some("abc".into()),
            url: "https://example.com/git.tar.gz".into(),
            source: "registry".into(),
        });
        assert!(lock.find("git").is_some());
        assert_eq!(lock.entries.len(), 1);
    }

    #[test]
    fn lockfile_upsert_replaces_existing() {
        let mut lock = LockFile::default();
        lock.upsert(LockEntry {
            name: "git".into(),
            version: "1.0.0".into(),
            checksum_sha256: "old".into(),
            installed_checksum_sha256: Some("old".into()),
            url: "u".into(),
            source: "registry".into(),
        });
        lock.upsert(LockEntry {
            name: "git".into(),
            version: "1.1.0".into(),
            checksum_sha256: "new".into(),
            installed_checksum_sha256: Some("new".into()),
            url: "u2".into(),
            source: "registry".into(),
        });
        assert_eq!(lock.entries.len(), 1);
        assert_eq!(lock.find("git").unwrap().version, "1.1.0");
    }

    #[test]
    fn lockfile_remove() {
        let mut lock = LockFile::default();
        lock.upsert(LockEntry {
            name: "git".into(),
            version: "1.0.0".into(),
            checksum_sha256: "x".into(),
            installed_checksum_sha256: Some("x".into()),
            url: "u".into(),
            source: "registry".into(),
        });
        assert!(lock.remove("git"));
        assert!(lock.find("git").is_none());
        assert!(!lock.remove("git")); // already gone
    }

    #[test]
    fn lockfile_roundtrips_toml() {
        let mut lock = LockFile::default();
        lock.upsert(LockEntry {
            name: "git".into(),
            version: "1.0.0".into(),
            checksum_sha256: "abc".into(),
            installed_checksum_sha256: Some("abc".into()),
            url: "https://x.com/git.tar.gz".into(),
            source: "registry".into(),
        });
        let serialized = toml::to_string_pretty(&lock).unwrap();
        let parsed: LockFile = toml::from_str(&serialized).unwrap();
        assert_eq!(lock, parsed);
    }
}
