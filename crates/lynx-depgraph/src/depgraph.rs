use lynx_core::error::{LynxError, Result};
use lynx_manifest::schema::PluginManifest;
use std::collections::{HashMap, HashSet, VecDeque};

/// Result of resolving a dep graph.
#[derive(Debug)]
pub struct LoadOrder {
    /// Plugins to load eagerly, in dependency order (deps first).
    pub eager: Vec<String>,
    /// Plugins to load lazily, in dependency order.
    pub lazy: Vec<String>,
    /// Plugins excluded due to missing binary deps: (plugin_name, missing_binary).
    pub excluded: Vec<(String, String)>,
}

/// Build a topological load order from a list of manifests.
///
/// - Uses Kahn's algorithm.
/// - Circular deps → `LynxError::Plugin` with full cycle path.
/// - Missing binary deps → excluded from order (not a hard error).
/// - Plugin dep references that are not in the manifests list are treated as
///   satisfied (they may be built-in or provided externally).
pub fn resolve(manifests: &[PluginManifest]) -> Result<LoadOrder> {
    // Check binary deps — exclude plugins whose required binaries are missing
    let mut excluded: Vec<(String, String)> = Vec::new();
    let mut excluded_names: HashSet<String> = HashSet::new();
    for m in manifests {
        for bin in &m.deps.binaries {
            if which(bin).is_none() {
                excluded.push((m.plugin.name.clone(), bin.clone()));
                excluded_names.insert(m.plugin.name.clone());
                break; // one missing binary is enough to exclude the plugin
            }
        }
    }

    // Build adjacency list (plugin → its dependencies that are in the manifests list)
    // Only include non-excluded plugins
    let active: Vec<&PluginManifest> = manifests
        .iter()
        .filter(|m| !excluded_names.contains(&m.plugin.name))
        .collect();

    let active_names: HashSet<&str> = active.iter().map(|m| m.plugin.name.as_str()).collect();

    // in-degree and adjacency for Kahn's
    let mut in_degree: HashMap<&str, usize> =
        active.iter().map(|m| (m.plugin.name.as_str(), 0)).collect();

    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new(); // dep → plugins that need it

    for m in &active {
        for dep in &m.deps.plugins {
            if active_names.contains(dep.as_str()) {
                *in_degree.entry(m.plugin.name.as_str()).or_insert(0) += 1;
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(m.plugin.name.as_str());
            }
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(&n, _)| n)
        .collect();

    let mut sorted: Vec<&str> = Vec::new();

    while let Some(node) = queue.pop_front() {
        sorted.push(node);
        if let Some(deps) = dependents.get(node) {
            for &dep in deps {
                let d = in_degree.entry(dep).or_insert(0);
                *d = d.saturating_sub(1);
                if *d == 0 {
                    queue.push_back(dep);
                }
            }
        }
    }

    // If not all active plugins were sorted, there's a cycle
    if sorted.len() < active.len() {
        let unsorted: HashSet<&str> = active
            .iter()
            .map(|m| m.plugin.name.as_str())
            .filter(|n| !sorted.contains(n))
            .collect();
        let cycle_path = find_cycle(&active, &unsorted);
        return Err(LynxError::Plugin(format!(
            "circular plugin dependency detected: {cycle_path}"
        )));
    }

    // Split into eager / lazy using the manifest's load.lazy flag
    let lazy_set: HashSet<&str> = active
        .iter()
        .filter(|m| m.load.lazy)
        .map(|m| m.plugin.name.as_str())
        .collect();

    let eager: Vec<String> = sorted
        .iter()
        .filter(|&&n| !lazy_set.contains(n))
        .map(|&n| n.to_string())
        .collect();

    let lazy: Vec<String> = sorted
        .iter()
        .filter(|&&n| lazy_set.contains(n))
        .map(|&n| n.to_string())
        .collect();

    Ok(LoadOrder {
        eager,
        lazy,
        excluded,
    })
}

/// Attempt to find and describe a cycle among the unsorted nodes (for error messages).
fn find_cycle<'a>(manifests: &[&'a PluginManifest], unsorted: &HashSet<&'a str>) -> String {
    // Simple DFS to find one cycle path
    for start in unsorted.iter() {
        let mut path: Vec<&str> = Vec::new();
        let mut visited: HashSet<&str> = HashSet::new();
        if let Some(cycle) = dfs_cycle(start, manifests, unsorted, &mut visited, &mut path) {
            return cycle;
        }
    }
    unsorted.iter().cloned().collect::<Vec<_>>().join(" → ")
}

fn dfs_cycle<'a>(
    node: &'a str,
    manifests: &[&'a PluginManifest],
    unsorted: &HashSet<&'a str>,
    visited: &mut HashSet<&'a str>,
    path: &mut Vec<&'a str>,
) -> Option<String> {
    if visited.contains(node) {
        // Found cycle — trim path to the cycle itself
        if let Some(pos) = path.iter().position(|&n| n == node) {
            let cycle: Vec<&str> = path[pos..].to_vec();
            return Some(format!("{} → {}", cycle.join(" → "), node));
        }
        return Some(node.to_string());
    }
    visited.insert(node);
    path.push(node);

    if let Some(m) = manifests.iter().find(|m| m.plugin.name == node) {
        for dep in &m.deps.plugins {
            if unsorted.contains(dep.as_str()) {
                if let Some(cycle) = dfs_cycle(dep, manifests, unsorted, visited, path) {
                    return Some(cycle);
                }
            }
        }
    }
    path.pop();
    None
}

fn which(bin: &str) -> Option<std::path::PathBuf> {
    lynx_core::paths::find_binary(bin)
}

// ── helpers for tests ────────────────────────────────────────────────────────

#[cfg(test)]
pub fn make_manifest(name: &str, deps: &[&str], lazy: bool, binaries: &[&str]) -> PluginManifest {
    use lynx_manifest::schema::*;
    PluginManifest {
        schema_version: 1,
        plugin: PluginMeta {
            name: name.into(),
            version: "0.1.0".into(),
            description: String::new(),
            authors: vec![],
        },
        load: LoadConfig {
            lazy,
            hooks: vec![],
        },
        deps: DepsConfig {
            binaries: binaries.iter().map(|s| s.to_string()).collect(),
            plugins: deps.iter().map(|s| s.to_string()).collect(),
        },
        exports: ExportsConfig::default(),
        contexts: ContextsConfig::default(),
        state: StateConfig::default(),
        shell: ShellConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_chain_loads_deps_first() {
        // C depends on B, B depends on A → sorted: A, B, C
        let manifests = vec![
            make_manifest("a", &[], false, &[]),
            make_manifest("b", &["a"], false, &[]),
            make_manifest("c", &["b"], false, &[]),
        ];
        let order = resolve(&manifests).unwrap();
        let pos: HashMap<_, _> = order
            .eager
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();
        assert!(pos["a"] < pos["b"]);
        assert!(pos["b"] < pos["c"]);
        assert!(order.lazy.is_empty());
        assert!(order.excluded.is_empty());
    }

    #[test]
    fn circular_dep_returns_error() {
        let manifests = vec![
            make_manifest("a", &["b"], false, &[]),
            make_manifest("b", &["a"], false, &[]),
        ];
        let err = resolve(&manifests).unwrap_err();
        assert!(err.to_string().contains("circular"));
    }

    #[test]
    fn missing_binary_excludes_plugin() {
        let manifests = vec![
            make_manifest("good", &[], false, &[]),
            make_manifest("bad", &[], false, &["__nonexistent_binary_xyz__"]),
        ];
        let order = resolve(&manifests).unwrap();
        assert!(order.eager.contains(&"good".to_string()));
        assert!(!order.eager.contains(&"bad".to_string()));
        assert_eq!(order.excluded.len(), 1);
        assert_eq!(order.excluded[0].0, "bad");
    }

    #[test]
    fn lazy_plugin_goes_to_lazy_list() {
        let manifests = vec![
            make_manifest("eager_one", &[], false, &[]),
            make_manifest("lazy_one", &[], true, &[]),
        ];
        let order = resolve(&manifests).unwrap();
        assert!(order.eager.contains(&"eager_one".to_string()));
        assert!(order.lazy.contains(&"lazy_one".to_string()));
        assert!(!order.eager.contains(&"lazy_one".to_string()));
    }
}
