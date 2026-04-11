use lynx_manifest::schema::PluginManifest;
use std::collections::HashMap;

/// The lifecycle stage a plugin is currently in.
#[derive(Debug, Clone, PartialEq)]
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
