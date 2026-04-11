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
                eprintln!("[lynx] warn: failed to parse {:?}: {}", manifest_path, e);
            }
        }
    }
    registry
}

/// RESOLVE stage: apply context filter and record excluded plugins.
///
/// The dep graph sort is done by the assembler (lynx-cli) using lynx-loader,
/// then the ordered names are passed back in here to mark states.
/// This keeps lynx-plugin free of a sideways dep on lynx-loader (D-001).
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
                reason: format!("missing binary: {}", bin),
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
