use crate::types::Event;
use lynx_core::runtime::socket_path;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Wire message format: newline-delimited JSON over the Unix socket.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "msg_type", rename_all = "snake_case")]
pub enum IpcMessage {
    /// Emit an event into the daemon's event bus.
    Emit { name: String, data: String },
    /// Register a zsh function as a subscriber for an event.
    Subscribe { event_name: String, zsh_fn: String },
}

impl IpcMessage {
    pub fn emit(event: &Event) -> Self {
        Self::Emit {
            name: event.name.clone(),
            data: event.data.clone(),
        }
    }
    pub fn subscribe(event_name: impl Into<String>, zsh_fn: impl Into<String>) -> Self {
        Self::Subscribe {
            event_name: event_name.into(),
            zsh_fn: zsh_fn.into(),
        }
    }
}

/// Send an IPC message to the running daemon's event socket.
///
/// Fire-and-forget: returns `Ok(())` immediately if the daemon is not running
/// (socket does not exist or connection is refused). Never blocks the caller.
pub fn send_ipc(msg: &IpcMessage) -> anyhow::Result<()> {
    let path = socket_path().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    if !path.exists() {
        // Daemon not running — silent no-op (D-001: shell side is fire-and-forget)
        return Ok(());
    }

    let mut stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(_) => return Ok(()), // daemon not running or socket stale
    };

    // Short write timeout so we never block the shell
    let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));

    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    let _ = stream.write_all(line.as_bytes()); // ignore write errors — fire-and-forget

    Ok(())
}

/// Emit an event to the daemon via IPC. Silent if daemon is not running.
pub fn emit_event(event: &Event) -> anyhow::Result<()> {
    send_ipc(&IpcMessage::emit(event))
}

/// Register a zsh function as a subscriber for the given event name.
pub fn register_subscriber(event_name: &str, zsh_fn: &str) -> anyhow::Result<()> {
    send_ipc(&IpcMessage::subscribe(event_name, zsh_fn))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Event, SHELL_CHPWD};

    #[test]
    fn ipc_message_serializes_to_json() {
        let msg = IpcMessage::emit(&Event::new(SHELL_CHPWD, "/home/user"));
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("shell:chpwd"));
        assert!(json.contains("/home/user"));
    }

    #[test]
    fn ipc_message_subscribe_serializes() {
        let msg = IpcMessage::subscribe(SHELL_CHPWD, "_my_handler");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("_my_handler"));
    }

    #[test]
    fn emit_event_no_daemon_is_silent() {
        // No daemon running — should return Ok silently
        std::env::set_var("LYNX_RUNTIME_DIR", "/tmp/lynx-bridge-test-no-daemon");
        let result = emit_event(&Event::named(SHELL_CHPWD));
        std::env::remove_var("LYNX_RUNTIME_DIR");
        assert!(result.is_ok());
    }
}
