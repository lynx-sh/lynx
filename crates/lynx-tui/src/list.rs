//! Interactive scrollable list component.
//!
//! Renders a themed, keyboard-navigable list using ratatui + crossterm.
//! Handles terminal setup/teardown, resize, and clean exit.

use std::io;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List as RatatuiList, ListItem as RatatuiListItem, ListState},
};

use crate::item::{ListItem, TuiColors};

/// Result of running the interactive list.
pub enum ListResult {
    /// User quit without selecting (q/esc).
    Cancelled,
    /// User selected an item (enter). Contains the index.
    Selected(usize),
}

/// Run an interactive list in the terminal.
///
/// Takes ownership of stdout, enters alternate screen + raw mode, runs the
/// event loop, then restores the terminal on exit (including on panic).
///
/// Returns `ListResult::Selected(index)` if the user pressed enter,
/// or `ListResult::Cancelled` if they quit.
pub fn run<T: ListItem>(
    items: &[T],
    title: &str,
    colors: &TuiColors,
) -> io::Result<ListResult> {
    if items.is_empty() {
        return Ok(ListResult::Cancelled);
    }

    // Setup terminal.
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run event loop — catch panics to restore terminal.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        event_loop(&mut terminal, items, title, colors)
    }));

    // Teardown — always runs.
    terminal::disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    match result {
        Ok(inner) => inner,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// Core event loop: render + handle input.
fn event_loop<T: ListItem>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    items: &[T],
    title: &str,
    colors: &TuiColors,
) -> io::Result<ListResult> {
    let mut state = ListState::default();
    state.select(Some(0));

    loop {
        terminal.draw(|frame| {
            render_list(frame, frame.area(), items, title, colors, &mut state);
        })?;

        // Block for next event.
        if let Event::Key(key) = event::read()? {
            // Only handle key press events (not release/repeat).
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    return Ok(ListResult::Cancelled);
                }
                KeyCode::Enter => {
                    if let Some(idx) = state.selected() {
                        return Ok(ListResult::Selected(idx));
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = state.selected().unwrap_or(0);
                    let next = if i >= items.len() - 1 { 0 } else { i + 1 };
                    state.select(Some(next));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = state.selected().unwrap_or(0);
                    let prev = if i == 0 { items.len() - 1 } else { i - 1 };
                    state.select(Some(prev));
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    state.select(Some(0));
                }
                KeyCode::End | KeyCode::Char('G') => {
                    state.select(Some(items.len() - 1));
                }
                _ => {}
            }
        }
    }
}

/// Render the list widget into a frame area.
fn render_list<T: ListItem>(
    frame: &mut Frame,
    area: Rect,
    items: &[T],
    title: &str,
    colors: &TuiColors,
    state: &mut ListState,
) {
    let count = items.len();

    // Build list items with styled text.
    let list_items: Vec<RatatuiListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = state.selected() == Some(i);
            let marker = if item.is_active() {
                "● "
            } else if is_selected {
                "› "
            } else {
                "  "
            };

            let subtitle = item.subtitle();
            let text = if subtitle.is_empty() {
                format!("{marker}{}", item.title())
            } else {
                format!("{marker}{}  {}", item.title(), subtitle)
            };

            let style = if is_selected {
                Style::default().fg(colors.accent).bold()
            } else if item.is_active() {
                Style::default().fg(colors.success)
            } else {
                Style::default()
            };

            RatatuiListItem::new(text).style(style)
        })
        .collect();

    let title_text = format!(" {title} ({count}) ");
    let help_text = " j/k navigate  enter select  q quit ";

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.muted))
        .title(title_text)
        .title_style(Style::default().fg(colors.accent).bold())
        .title_bottom(help_text)
        .title_alignment(Alignment::Left);

    let list = RatatuiList::new(list_items)
        .block(block)
        .highlight_symbol("") // We handle markers in the item text
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Never);

    frame.render_stateful_widget(list, area, state);
}

/// Print items as plain text (non-TTY fallback).
pub fn print_plain<T: ListItem>(items: &[T], title: &str) {
    println!("{title} ({} items)", items.len());
    for item in items {
        let marker = if item.is_active() { "●" } else { " " };
        let subtitle = item.subtitle();
        if subtitle.is_empty() {
            println!("  {marker} {}", item.title());
        } else {
            println!("  {marker} {} — {}", item.title(), subtitle);
        }
    }
}

/// Show items interactively if TTY, or as plain text if piped/agent.
///
/// This is the single entry point all CLI commands should use.
pub fn show<T: ListItem>(
    items: &[T],
    title: &str,
    colors: &TuiColors,
) -> io::Result<Option<usize>> {
    // Non-interactive fallback: pipe, CI, or agent context.
    if !atty_stdout() {
        print_plain(items, title);
        return Ok(None);
    }

    match run(items, title, colors)? {
        ListResult::Selected(idx) => Ok(Some(idx)),
        ListResult::Cancelled => Ok(None),
    }
}

/// Check if stdout is a TTY.
fn atty_stdout() -> bool {
    crossterm::tty::IsTty::is_tty(&io::stdout())
}
