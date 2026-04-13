//! Agent context detection for lynx-workflow.
//!
//! Single delegation point — all phases that need to know "are we in an agent session?"
//! call `is_agent_context()` here. Never read CLAUDECODE/CURSOR_CLI directly in this crate.

use lynx_core::types::Context;
use lynx_shell::context::detect_context;

/// Returns `true` when the current process is running inside an agent session
/// (Claude Code, Cursor CLI, or `LYNX_CONTEXT=agent`).
///
/// Delegates entirely to [`lynx_shell::context::detect_context`] — no duplicated
/// env-var logic in this crate.
pub fn is_agent_context() -> bool {
    detect_context() == Context::Agent
}
