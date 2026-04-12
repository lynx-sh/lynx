//! Centralized error renderer for all `lx` commands.
//!
//! # Usage
//!
//! Call `render_error(&err)` in `main.rs` when a command returns `Err`.
//! It handles both typed `LynxError` variants and raw `anyhow` errors.
//!
//! # Output format
//!
//! Typed `LynxError` (color terminal):
//! ```text
//!  error  Plugin 'git-extras' is not installed
//!   hint  run `lx plugin add git-extras` to install it
//! ```
//!
//! Raw/untyped error:
//! ```text
//!  error  TOML parse error at line 4: unexpected key
//! ```
//!
//! # For agents adding new errors
//!
//! See `patterns/error-protocol.md` — always use `LynxError::*` variants
//! for user-facing errors. Raw `bail!()` works but produces no hint line.

use anyhow::Error;
use lynx_core::error::LynxError;
use owo_colors::OwoColorize;

/// Render an error to stdout with color and structure.
///
/// Uses stdout (not stderr) so it stays visible even when the Lynx shell
/// precmd hook re-renders the prompt (which can overwrite stderr).
pub fn render_error(err: &Error) {
    if let Some(lynx_err) = err.downcast_ref::<LynxError>() {
        render_lynx_error(lynx_err);
    } else {
        render_plain_error(err);
    }
}

fn render_lynx_error(err: &LynxError) {
    let message = err.message();
    if color_enabled() {
        println!("{}  {message}", " error ".on_red().white().bold());
        if let Some(hint) = err.hint() {
            println!("{}  {}", "  hint ".bold().yellow(), hint.dimmed());
        }
    } else {
        println!(" error   {message}");
        if let Some(hint) = err.hint() {
            println!("  hint   {hint}");
        }
    }
}

fn render_plain_error(err: &Error) {
    // Use the full anyhow chain ({err:#}) for context; no hint available.
    if color_enabled() {
        println!("{}  {err:#}", " error ".on_red().white().bold());
    } else {
        println!(" error   {err:#}");
    }
}

/// Returns true if color output should be used.
///
/// Respects NO_COLOR (https://no-color.org) and TERM=dumb conventions.
fn color_enabled() -> bool {
    use std::io::IsTerminal;
    std::env::var_os("NO_COLOR").is_none()
        && std::env::var_os("TERM").as_deref() != Some(std::ffi::OsStr::new("dumb"))
        && std::io::stdout().is_terminal()
}
