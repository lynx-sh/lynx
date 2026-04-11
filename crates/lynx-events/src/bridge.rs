use serde::{Deserialize, Serialize};

/// Wire message format: newline-delimited JSON over the Unix socket.
///
/// Used for daemon ↔ task-scheduler communication only.
/// Event dispatch is in-process via bus::build_active_bus() in lx commands — not IPC.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "msg_type", rename_all = "snake_case")]
pub enum IpcMessage {
    /// Emit an event into the daemon's task bus.
    Emit { name: String, data: String },
    /// Register a zsh function as a subscriber for an event.
    Subscribe { event_name: String, zsh_fn: String },
}

impl IpcMessage {
    pub fn emit(name: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Emit {
            name: name.into(),
            data: data.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_message_emit_serializes() {
        let msg = IpcMessage::emit("task:completed", "backup");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("task:completed"));
        assert!(json.contains("backup"));
    }
}
