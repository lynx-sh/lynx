//! PTY-based command runner for interactive foreground streaming.
//!
//! Used only when: agent_mode == false AND stream_tx is Some.
//! PTY gives the child process a real terminal, enabling programs that buffer
//! output when stdout is piped (e.g. grep, cargo) to flush line-by-line.
//!
//! PTY output does not distinguish stdout/stderr — both arrive on the master.
//! All lines are emitted as `StepOutput { is_stderr: false }`.

use crate::executor::{StreamEvent, STEP_OUTPUT_LINE_CAP};
use crate::runner::ResolvedCommand;
use crate::schema::Step;

/// Run `cmd` inside a PTY, streaming each output line to `tx`.
///
/// Returns the child exit code on success, or `Err(())` on spawn or PTY
/// allocation failure. PTY allocation failure also triggers a graceful
/// fallback in the caller — see `run_command` in executor.rs.
pub async fn run_in_pty(
    cmd: &ResolvedCommand,
    step: &Step,
    effective_cwd: Option<&str>,
    tx: &std::sync::mpsc::Sender<StreamEvent>,
    step_name: &str,
) -> Result<(i32, Vec<String>, Vec<String>), ()> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};

    let pty_system = native_pty_system();

    let pair = match pty_system.openpty(PtySize {
        rows: 24,
        cols: 200,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(step = %step_name, "PTY allocation failed ({}); falling back to piped I/O", e);
            return Err(());
        }
    };

    let mut builder = CommandBuilder::new(&cmd.binary);
    for arg in &cmd.args {
        builder.arg(arg);
    }
    if let Some(cwd) = effective_cwd {
        builder.cwd(cwd);
    }
    for (k, v) in &step.env {
        builder.env(k, v);
    }

    let mut child = match pair.slave.spawn_command(builder) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(step = %step_name, "PTY spawn failed: {e}");
            return Err(());
        }
    };

    // Drop the slave end so the master gets EOF when the child exits.
    drop(pair.slave);

    let reader = match pair.master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(step = %step_name, "PTY reader clone failed: {e}");
            return Err(());
        }
    };

    let name = step_name.to_string();
    let tx_clone = tx.clone();

    // Read loop runs in a blocking task — PTY master is a sync Read.
    let read_handle = tokio::task::spawn_blocking(move || {
        use std::io::{BufRead, BufReader};
        let mut collected: Vec<String> = Vec::new();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    // Strip the trailing newline that PTY appends.
                    let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
                    let _ = tx_clone.send(StreamEvent::StepOutput {
                        name: name.clone(),
                        line: trimmed.clone(),
                        is_stderr: false,
                    });
                    if collected.len() < STEP_OUTPUT_LINE_CAP {
                        collected.push(trimmed);
                    } else if collected.len() == STEP_OUTPUT_LINE_CAP {
                        tracing::warn!(
                            step = %name,
                            "PTY output exceeded {STEP_OUTPUT_LINE_CAP} lines; dropping further lines from log buffer"
                        );
                    }
                }
                Err(_) => break, // PTY closed (normal on child exit)
            }
        }
        collected
    });

    // Apply timeout if configured.
    let exit_result: Result<i32, ()> = if let Some(timeout_sec) = step.timeout_sec {
        let timeout = std::time::Duration::from_secs(timeout_sec);
        match tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || child.wait()))
            .await
        {
            Ok(Ok(Ok(status))) => Ok(status.exit_code() as i32),
            _ => {
                let _ = tx.send(StreamEvent::StepOutput {
                    name: step_name.to_string(),
                    line: format!("timed out after {timeout_sec}s"),
                    is_stderr: true,
                });
                Err(())
            }
        }
    } else {
        match tokio::task::spawn_blocking(move || child.wait()).await {
            Ok(Ok(status)) => Ok(status.exit_code() as i32),
            _ => Err(()),
        }
    };

    let out = read_handle.await.unwrap_or_default();

    exit_result.map(|code| (code, out, vec![]))
}
