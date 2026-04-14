use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

/// Set up the alternate-screen terminal, run `f`, then unconditionally clean up.
///
/// Handles raw mode, alternate screen, mouse capture, and cursor restoration.
/// Panics inside `f` are caught and re-raised after cleanup.
pub fn with_terminal<F, R>(f: F) -> io::Result<R>
where
    F: FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<R>,
{
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    // SAFETY: we assert the closure is unwind-safe so that cleanup always runs.
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut term)));

    terminal::disable_raw_mode().ok();
    execute!(term.backend_mut(), DisableMouseCapture, LeaveAlternateScreen).ok();
    term.show_cursor().ok();

    match result {
        Ok(inner) => inner,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
