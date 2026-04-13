use std::path::PathBuf;

/// Resolve a plugin directory by name.
///
/// Search order:
/// 1. Installed plugin directory (`~/.config/lynx/plugins/<name>`)
/// 2. In-repo development plugin directory (`./plugins/<name>`)
pub(super) fn resolve_plugin_dir(name: &str) -> Option<PathBuf> {
    let installed = lynx_core::paths::installed_plugins_dir().join(name);
    if installed.exists() {
        return Some(installed);
    }

    let repo = PathBuf::from("plugins").join(name);
    if repo.exists() {
        return Some(repo);
    }

    None
}
