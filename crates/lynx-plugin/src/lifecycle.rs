use crate::context_filter::filter_for_context;
use crate::registry::{PluginEntry, PluginRegistry, PluginState};
use lynx_core::error::Result;
use lynx_core::types::Context;
use lynx_events::EventBus;
use lynx_manifest::schema::PluginManifest;
use std::path::Path;
use std::sync::Arc;

/// DECLARE stage: parse all plugin.toml files from a directory.
///
/// Returns a registry with all successfully parsed plugins in `Declared` state.
/// Parse failures do not block other plugins.
pub fn declare(plugins_dir: &Path) -> PluginRegistry {
    let mut registry = PluginRegistry::new();

    let entries = match std::fs::read_dir(plugins_dir) {
        Ok(e) => e,
        Err(_) => return registry,
    };

    for entry in entries.flatten() {
        let manifest_path = entry.path().join(lynx_core::brand::PLUGIN_MANIFEST);
        if !manifest_path.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&manifest_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        match lynx_manifest::parse(&content) {
            Ok(manifest) => {
                let mut plugin_entry = PluginEntry::new(manifest);
                plugin_entry.plugin_dir = Some(entry.path());
                registry.insert(plugin_entry);
            }
            Err(e) => {
                tracing::warn!("failed to parse {manifest_path:?}: {e}");
            }
        }
    }
    registry
}

/// RESOLVE stage: apply context filter and record excluded plugins.
///
/// The dep graph sort is done by the assembler (lynx-cli) using lynx-depgraph,
/// then the ordered names are passed back in here to mark states.
/// This keeps lynx-plugin free of a sideways dep on lynx-depgraph (D-001).
pub fn apply_resolve(
    registry: &mut PluginRegistry,
    context: &Context,
    eager_order: &[String],
    lazy_order: &[String],
    excluded: &[(String, String)],
) {
    let all_manifests: Vec<PluginManifest> = registry.all().map(|e| e.manifest.clone()).collect();
    let (_, disabled) = filter_for_context(&all_manifests, context);

    for (name, reason) in &disabled {
        registry.set_state(
            name,
            PluginState::Excluded {
                reason: reason.clone(),
            },
        );
    }
    for (name, bin) in excluded {
        registry.set_state(
            name,
            PluginState::Excluded {
                reason: format!("missing binary: {bin}"),
            },
        );
    }
    for name in eager_order.iter().chain(lazy_order.iter()) {
        registry.set_state(name, PluginState::Resolved);
    }
}

/// Map a plugin.toml short hook name to the canonical EventBus event name.
///
/// Two subscription pathways exist:
/// - **Shell-side (working now):** `lx init` output wires `_pluginname_hook()` functions
///   into zsh hook arrays (chpwd_functions, precmd_functions, etc.). This is what
///   actually runs today.
/// - **Daemon-side (this function):** registers Rust handlers on the EventBus so the
///   daemon can act when events arrive over IPC. NOTE: `activate()` is not yet called
///   by the daemon — see wiring issue filed against lynx-daemon.
///
/// Both pathways are valid; they serve different purposes:
/// - Shell hooks: run zsh-side plugin code during prompt render cycle
/// - EventBus handlers: run Rust-side daemon logic (caching, side effects)
fn hook_event_name(hook: &str) -> String {
    match hook {
        "chpwd" => "shell:chpwd".to_string(),
        "precmd" => "shell:precmd".to_string(),
        "preexec" => "shell:preexec".to_string(),
        other => format!("shell:{other}"),
    }
}

/// ACTIVATE stage: register EventBus subscriptions from the manifest's hooks list.
///
/// Handlers trace the dispatch so daemon-side hook delivery is observable.
/// Shell-side hook execution (the working path today) is a separate mechanism
/// wired by `lx init` output — see `hook_event_name` doc above.
pub fn activate(name: &str, manifest: &PluginManifest, bus: Arc<EventBus>) -> Result<()> {
    for hook in &manifest.load.hooks {
        let event_name = hook_event_name(hook);
        let plugin_name = name.to_string();
        let hook_short = hook.clone();
        bus.subscribe(&event_name, move |ev| {
            let plugin = plugin_name.clone();
            let hook = hook_short.clone();
            async move {
                tracing::debug!(plugin = %plugin, hook = %hook, data = %ev.data,
                    "plugin hook dispatched via EventBus");
            }
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_event_name_known_hooks() {
        assert_eq!(hook_event_name("chpwd"), "shell:chpwd");
        assert_eq!(hook_event_name("precmd"), "shell:precmd");
        assert_eq!(hook_event_name("preexec"), "shell:preexec");
    }

    #[test]
    fn hook_event_name_unknown_prefixes_shell() {
        assert_eq!(hook_event_name("custom_hook"), "shell:custom_hook");
        assert_eq!(hook_event_name("zshaddhistory"), "shell:zshaddhistory");
    }

    #[test]
    fn declare_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = declare(tmp.path());
        assert_eq!(reg.all().count(), 0);
    }

    #[test]
    fn declare_nonexistent_dir() {
        let reg = declare(std::path::Path::new("/nonexistent/plugins"));
        assert_eq!(reg.all().count(), 0);
    }

    #[test]
    fn declare_valid_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_dir = tmp.path().join("test-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.toml"), r#"
            [plugin]
            name = "test-plugin"
            version = "1.0.0"
            [load]
            [deps]
            [exports]
            [contexts]
        "#).unwrap();

        let reg = declare(tmp.path());
        assert_eq!(reg.all().count(), 1);
        let entry = reg.get("test-plugin").unwrap();
        assert_eq!(entry.manifest.plugin.name, "test-plugin");
        assert!(entry.plugin_dir.is_some());
    }

    #[test]
    fn declare_skips_invalid_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_dir = tmp.path().join("bad");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.toml"), "not valid toml {{{").unwrap();

        let reg = declare(tmp.path());
        assert_eq!(reg.all().count(), 0);
    }

    #[test]
    fn declare_skips_dir_without_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("no-manifest")).unwrap();

        let reg = declare(tmp.path());
        assert_eq!(reg.all().count(), 0);
    }
}
