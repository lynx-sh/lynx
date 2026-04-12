use serde::{Deserialize, Serialize};

/// Install source for a locked plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PluginSource {
    #[default]
    Registry,
    Local,
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
    /// Install method: registry or local.
    #[serde(default)]
    pub source: PluginSource,
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

    #[test]
    fn lockfile_upsert_and_find() {
        let mut lock = LockFile::default();
        lock.upsert(LockEntry {
            name: "git".into(),
            version: "1.0.0".into(),
            checksum_sha256: "abc".into(),
            installed_checksum_sha256: Some("abc".into()),
            url: "https://example.com/git.tar.gz".into(),
            source: PluginSource::Registry,
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
            source: PluginSource::Registry,
        });
        lock.upsert(LockEntry {
            name: "git".into(),
            version: "1.1.0".into(),
            checksum_sha256: "new".into(),
            installed_checksum_sha256: Some("new".into()),
            url: "u2".into(),
            source: PluginSource::Registry,
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
            source: PluginSource::Registry,
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
            source: PluginSource::Registry,
        });
        let serialized = toml::to_string_pretty(&lock).unwrap();
        let parsed: LockFile = toml::from_str(&serialized).unwrap();
        assert_eq!(lock, parsed);
    }

    #[test]
    fn source_enum_serializes_as_lowercase() {
        let entry = LockEntry {
            name: "test".into(),
            version: "1.0.0".into(),
            checksum_sha256: "x".into(),
            installed_checksum_sha256: None,
            url: "u".into(),
            source: PluginSource::Local,
        };
        let s = toml::to_string_pretty(&entry).unwrap();
        assert!(s.contains("source = \"local\""));
    }

    #[test]
    fn source_enum_deserializes_from_string() {
        let toml_str = r#"
name = "test"
version = "1.0.0"
checksum_sha256 = "x"
url = "u"
source = "registry"
"#;
        let entry: LockEntry = toml::from_str(toml_str).unwrap();
        assert_eq!(entry.source, PluginSource::Registry);
    }

    #[test]
    fn source_defaults_to_registry() {
        let toml_str = r#"
name = "test"
version = "1.0.0"
checksum_sha256 = "x"
url = "u"
"#;
        let entry: LockEntry = toml::from_str(toml_str).unwrap();
        assert_eq!(entry.source, PluginSource::Registry);
    }
}
