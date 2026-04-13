/// In-process EventBus builder.
///
/// Every `lx` command that emits events calls this to get a fully activated bus.
/// The lifecycle runs in-process — declare → resolve → activate — and the bus
/// is discarded when the process exits. No daemon required.
use lynx_core::types::Context;
use lynx_events::EventBus;
use lynx_manifest::schema::PluginManifest;
use lynx_plugin::{lifecycle, registry::PluginState};
use std::path::Path;
use std::sync::Arc;

/// Build an in-process EventBus with all resolved plugins activated.
///
/// Plugins that fail to declare or activate are skipped with a warning;
/// they never block the bus from being returned.
pub fn build_active_bus(context: &Context, plugins_dir: &Path) -> Arc<EventBus> {
    let bus = Arc::new(EventBus::new());

    let mut registry = lifecycle::declare(plugins_dir);

    let manifests: Vec<PluginManifest> = registry.all().map(|e| e.manifest.clone()).collect();

    let load_order = match lynx_depgraph::depgraph::resolve(&manifests) {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("plugin dep resolution failed — event handlers not registered: {e}");
            return bus;
        }
    };

    lifecycle::apply_resolve(
        &mut registry,
        context,
        &load_order.eager,
        &load_order.lazy,
        &load_order.excluded,
    );

    for entry in registry.all() {
        if matches!(entry.state, PluginState::Resolved) {
            if let Err(e) = lifecycle::activate(
                &entry.manifest.plugin.name,
                &entry.manifest,
                Arc::clone(&bus),
            ) {
                tracing::warn!(plugin = %entry.manifest.plugin.name, "activate failed: {e}");
            }
        }
    }

    bus
}
