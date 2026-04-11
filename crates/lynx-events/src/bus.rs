use crate::types::Event;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

type AsyncHandler = Arc<dyn Fn(Event) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// A simple async event bus.
///
/// Subscribers register handlers keyed by event name. `emit` dispatches an
/// event immediately to all matching handlers. All cross-plugin communication
/// must go through here (D-008).
pub struct EventBus {
    handlers: Arc<Mutex<HashMap<String, Vec<AsyncHandler>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register an async handler for the given event name.
    pub fn subscribe<F, Fut>(&self, event_name: &str, handler: F)
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let boxed: AsyncHandler = Arc::new(move |ev| Box::pin(handler(ev)));
        self.handlers
            .lock()
            .expect("event bus mutex poisoned")
            .entry(event_name.to_string())
            .or_default()
            .push(boxed);
    }

    /// Emit an event, running all registered handlers for its name.
    ///
    /// Handlers are called sequentially in registration order.
    /// Returns the number of handlers invoked.
    pub async fn emit(&self, event: Event) -> usize {
        let handlers: Vec<AsyncHandler> = {
            let lock = self.handlers.lock().expect("event bus mutex poisoned");
            match lock.get(&event.name) {
                None => return 0,
                Some(hs) => hs.clone(),
            }
        };

        let count = handlers.len();
        for handler in handlers {
            (handler)(event.clone()).await;
        }
        count
    }

    /// Alias for clarity — same as emit. Provided for API symmetry.
    pub async fn dispatch(&self, event: Event) -> usize {
        self.emit(event).await
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Event, SHELL_CHPWD};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn emit_reaches_subscriber() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        bus.subscribe(SHELL_CHPWD, move |_ev| {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        });
        bus.emit(Event::new(SHELL_CHPWD, "/home/user")).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn multiple_subscribers_all_called() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let c = counter.clone();
            bus.subscribe(SHELL_CHPWD, move |_| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
        bus.emit(Event::named(SHELL_CHPWD)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn unsubscribed_event_not_called() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        bus.subscribe("other:event", move |_| {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        });
        bus.emit(Event::named(SHELL_CHPWD)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn event_data_reaches_handler() {
        let bus = EventBus::new();
        let received = Arc::new(Mutex::new(String::new()));
        let r = received.clone();
        bus.subscribe(SHELL_CHPWD, move |ev| {
            let r = r.clone();
            async move {
                *r.lock().unwrap() = ev.data.clone();
            }
        });
        bus.emit(Event::new(SHELL_CHPWD, "/tmp/test")).await;
        assert_eq!(*received.lock().unwrap(), "/tmp/test");
    }
}
