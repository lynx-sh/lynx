use lynx_manifest::schema::PluginManifest;
use std::collections::HashMap;

/// The lifecycle stage a plugin is currently in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Pending,
    Declared,
    Resolved,
    Excluded { reason: String },
    Loaded,
    Active,
    Failed { reason: String },
    Degraded { reason: String }, // loaded but hooks broken
}

impl PluginState {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
    pub fn is_excluded(&self) -> bool {
        matches!(self, Self::Excluded { .. })
    }
}

/// Per-plugin entry in the registry.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub manifest: PluginManifest,
    pub state: PluginState,
    /// Wall-clock load time in milliseconds (set after LOAD stage).
    pub load_time_ms: Option<u64>,
    /// Absolute path to the plugin directory.
    pub plugin_dir: Option<std::path::PathBuf>,
}

impl PluginEntry {
    pub fn new(manifest: PluginManifest) -> Self {
        Self {
            manifest,
            state: PluginState::Declared,
            load_time_ms: None,
            plugin_dir: None,
        }
    }
}

/// Central registry tracking every known plugin's state.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    entries: HashMap<String, PluginEntry>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: PluginEntry) {
        self.entries
            .insert(entry.manifest.plugin.name.clone(), entry);
    }

    pub fn get(&self, name: &str) -> Option<&PluginEntry> {
        self.entries.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut PluginEntry> {
        self.entries.get_mut(name)
    }

    pub fn set_state(&mut self, name: &str, state: PluginState) {
        if let Some(e) = self.entries.get_mut(name) {
            e.state = state;
        }
    }

    pub fn all(&self) -> impl Iterator<Item = &PluginEntry> {
        self.entries.values()
    }

    pub fn names_in_state(&self, state: &PluginState) -> Vec<String> {
        self.entries
            .values()
            .filter(|e| &e.state == state)
            .map(|e| e.manifest.plugin.name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_manifest::schema::*;

    fn test_manifest(name: &str) -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            plugin: PluginMeta {
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: String::new(),
                authors: vec![],
            },
            load: LoadConfig::default(),
            deps: DepsConfig::default(),
            exports: ExportsConfig::default(),
            contexts: ContextsConfig::default(),
            state: StateConfig::default(),
            shell: ShellConfig::default(),
        }
    }

    #[test]
    fn plugin_state_checks() {
        assert!(PluginState::Active.is_active());
        assert!(!PluginState::Pending.is_active());
        assert!(PluginState::Failed { reason: "x".into() }.is_failed());
        assert!(!PluginState::Active.is_failed());
        assert!(PluginState::Excluded { reason: "x".into() }.is_excluded());
        assert!(!PluginState::Resolved.is_excluded());
    }

    #[test]
    fn plugin_entry_new_starts_declared() {
        let entry = PluginEntry::new(test_manifest("git"));
        assert_eq!(entry.state, PluginState::Declared);
        assert!(entry.load_time_ms.is_none());
        assert!(entry.plugin_dir.is_none());
    }

    #[test]
    fn registry_insert_and_get() {
        let mut reg = PluginRegistry::new();
        reg.insert(PluginEntry::new(test_manifest("git")));
        assert!(reg.get("git").is_some());
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn registry_set_state() {
        let mut reg = PluginRegistry::new();
        reg.insert(PluginEntry::new(test_manifest("git")));
        reg.set_state("git", PluginState::Active);
        assert!(reg.get("git").unwrap().state.is_active());
    }

    #[test]
    fn registry_set_state_nonexistent_is_noop() {
        let mut reg = PluginRegistry::new();
        reg.set_state("missing", PluginState::Active); // should not panic
    }

    #[test]
    fn registry_names_in_state() {
        let mut reg = PluginRegistry::new();
        reg.insert(PluginEntry::new(test_manifest("a")));
        reg.insert(PluginEntry::new(test_manifest("b")));
        reg.set_state("a", PluginState::Active);
        let active = reg.names_in_state(&PluginState::Active);
        assert_eq!(active, vec!["a"]);
    }

    #[test]
    fn registry_all_iterates_all_entries() {
        let mut reg = PluginRegistry::new();
        reg.insert(PluginEntry::new(test_manifest("x")));
        reg.insert(PluginEntry::new(test_manifest("y")));
        assert_eq!(reg.all().count(), 2);
    }

    #[test]
    fn registry_get_mut() {
        let mut reg = PluginRegistry::new();
        reg.insert(PluginEntry::new(test_manifest("git")));
        let entry = reg.get_mut("git").unwrap();
        entry.load_time_ms = Some(42);
        assert_eq!(reg.get("git").unwrap().load_time_ms, Some(42));
    }
}
