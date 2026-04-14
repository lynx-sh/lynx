/// In-process EventBus builder.
///
/// Builds a bus for commands that emit lifecycle events (theme change, context switch, etc.).
/// No plugin handlers are registered — handler wiring requires a future daemon-side design.
/// The bus is discarded when the process exits.
use lynx_events::EventBus;
use std::sync::Arc;

pub fn build_active_bus() -> Arc<EventBus> {
    Arc::new(EventBus::new())
}
