//! Interactive scrollable list component with search and preview.
//!
//! Renders a themed, keyboard-navigable list using ratatui + crossterm.
//! Handles terminal setup/teardown, resize, and clean exit.

use std::io;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List as RatatuiList, ListItem as RatatuiListItem, ListState},
};

use crate::item::{ListItem, TuiColors};
use crate::preview;

/// Result of running the interactive list.
pub enum ListResult {
    /// User quit without selecting (q/esc).
    Cancelled,
    /// User selected an item (enter). Contains the original index.
    Selected(usize),
}

/// Input mode for the list.
#[derive(PartialEq)]
enum Mode {
    /// Normal navigation mode.
    Normal,
    /// Search/filter mode — keystrokes go to the search input.
    Search,
}

/// Mutable state for the interactive list.
pub(crate) struct AppState {
    /// Current input mode.
    mode: Mode,
    /// ListState for the visible (filtered) list.
    pub(crate) list_state: ListState,
    /// Search query text.
    pub(crate) query: String,
    /// Indices into the original items vec that match the current filter.
    pub(crate) filtered: Vec<usize>,
}

impl AppState {
    fn new(total: usize) -> Self {
        let mut list_state = ListState::default();
        if total > 0 {
            list_state.select(Some(0));
        }
        Self {
            mode: Mode::Normal,
            list_state,
            query: String::new(),
            filtered: (0..total).collect(),
        }
    }

    /// Recompute filtered indices from the query.
    fn refilter<T: ListItem>(&mut self, items: &[T]) {
        if self.query.is_empty() {
            self.filtered = (0..items.len()).collect();
        } else {
            let q = self.query.to_lowercase();
            self.filtered = items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    item.title().to_lowercase().contains(&q)
                        || item.subtitle().to_lowercase().contains(&q)
                        || item.tags().iter().any(|t| t.to_lowercase().contains(&q))
                        || item
                            .category()
                            .map(|c| c.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
        }
        // Reset selection to first match.
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn move_down(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        let next = if i >= self.filtered.len() - 1 {
            0
        } else {
            i + 1
        };
        self.list_state.select(Some(next));
    }

    fn move_up(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        let prev = if i == 0 {
            self.filtered.len() - 1
        } else {
            i - 1
        };
        self.list_state.select(Some(prev));
    }

    /// Get the original index of the currently selected item.
    pub(crate) fn selected_original_index(&self) -> Option<usize> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered.get(i).copied())
    }
}

// ── Minimum terminal width to show preview pane ─────────────────────────────
const PREVIEW_MIN_WIDTH: u16 = 80;

/// Run an interactive list in the terminal.
pub fn run<T: ListItem>(items: &[T], title: &str, colors: &TuiColors) -> io::Result<ListResult> {
    if items.is_empty() {
        return Ok(ListResult::Cancelled);
    }

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        event_loop(&mut terminal, items, title, colors)
    }));

    terminal::disable_raw_mode()?;
    terminal.backend_mut().execute(DisableMouseCapture)?;
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
    let mut state = AppState::new(items.len());

    loop {
        terminal.draw(|frame| {
            render(frame, items, title, colors, &mut state);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match state.mode {
                Mode::Normal => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if !state.query.is_empty() {
                            // Clear search first, then quit on second press.
                            state.query.clear();
                            state.refilter(items);
                        } else {
                            return Ok(ListResult::Cancelled);
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(idx) = state.selected_original_index() {
                            return Ok(ListResult::Selected(idx));
                        }
                    }
                    KeyCode::Char('/') => {
                        state.mode = Mode::Search;
                    }
                    KeyCode::Down | KeyCode::Char('j') => state.move_down(),
                    KeyCode::Up | KeyCode::Char('k') => state.move_up(),
                    KeyCode::Home | KeyCode::Char('g') => {
                        if !state.filtered.is_empty() {
                            state.list_state.select(Some(0));
                        }
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        if !state.filtered.is_empty() {
                            state.list_state.select(Some(state.filtered.len() - 1));
                        }
                    }
                    _ => {}
                },
                Mode::Search => match key.code {
                    KeyCode::Esc => {
                        state.mode = Mode::Normal;
                    }
                    KeyCode::Enter => {
                        state.mode = Mode::Normal;
                    }
                    KeyCode::Backspace => {
                        state.query.pop();
                        state.refilter(items);
                    }
                    KeyCode::Char(c) => {
                        // Ctrl+C in search mode = quit.
                        if c == 'c' && key.modifiers.contains(KeyModifiers::CONTROL) {
                            return Ok(ListResult::Cancelled);
                        }
                        state.query.push(c);
                        state.refilter(items);
                    }
                    KeyCode::Down => state.move_down(),
                    KeyCode::Up => state.move_up(),
                    _ => {}
                },
            }
        }
    }
}

/// Top-level render: split into list + preview if wide enough.
fn render<T: ListItem>(
    frame: &mut Frame,
    items: &[T],
    title: &str,
    colors: &TuiColors,
    state: &mut AppState,
) {
    let area = frame.area();
    let show_preview = area.width >= PREVIEW_MIN_WIDTH;

    if show_preview {
        // Split: 45% list, 55% preview.
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);

        render_list(frame, chunks[0], items, title, colors, state);
        preview::render_preview(frame, chunks[1], items, colors, state);
    } else {
        render_list(frame, area, items, title, colors, state);
    }
}

/// Render the list pane.
fn render_list<T: ListItem>(
    frame: &mut Frame,
    area: Rect,
    items: &[T],
    title: &str,
    colors: &TuiColors,
    state: &mut AppState,
) {
    let total = items.len();
    let matched = state.filtered.len();

    // Build visible list items from filtered indices.
    let list_items: Vec<RatatuiListItem> = state
        .filtered
        .iter()
        .enumerate()
        .map(|(vi, &orig_idx)| {
            let item = &items[orig_idx];
            let is_selected = state.list_state.selected() == Some(vi);
            let marker = if item.is_active() {
                "● "
            } else if is_selected {
                "› "
            } else {
                "  "
            };

            let title_text = item.title();
            let subtitle = item.subtitle();

            // Build styled line with match highlighting.
            let line = if state.query.is_empty() {
                let text = if subtitle.is_empty() {
                    format!("{marker}{title_text}")
                } else {
                    format!("{marker}{title_text}  {subtitle}")
                };
                let style = item_style(is_selected, item.is_active(), colors);
                Line::from(Span::styled(text, style))
            } else {
                build_highlighted_line(
                    marker,
                    title_text,
                    &subtitle,
                    &state.query,
                    is_selected,
                    item.is_active(),
                    colors,
                )
            };

            RatatuiListItem::new(line)
        })
        .collect();

    // Title: show filter count when searching.
    let title_text = if state.query.is_empty() {
        format!(" {title} ({total}) ")
    } else {
        format!(" {title} ({matched}/{total}) ")
    };

    // Bottom bar: show search input or keybindings.
    let bottom = if state.mode == Mode::Search {
        format!(" /{} ", state.query)
    } else if !state.query.is_empty() {
        format!(" filter: {}  (/ edit  esc clear) ", state.query)
    } else {
        " j/k navigate  / search  enter select  q quit ".to_string()
    };

    let border_style = if state.mode == Mode::Search {
        Style::default().fg(colors.accent)
    } else {
        Style::default().fg(colors.muted)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title_text)
        .title_style(Style::default().fg(colors.accent).bold())
        .title_bottom(Line::from(bottom).style(Style::default().fg(colors.muted)))
        .title_alignment(Alignment::Left);

    let list = RatatuiList::new(list_items)
        .block(block)
        .highlight_symbol("")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Never);

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

/// Build a line with highlighted search matches.
fn build_highlighted_line(
    marker: &str,
    title: &str,
    subtitle: &str,
    query: &str,
    is_selected: bool,
    is_active: bool,
    colors: &TuiColors,
) -> Line<'static> {
    let base_style = item_style(is_selected, is_active, colors);
    let highlight_style = Style::default().fg(colors.warning).bold();

    let mut spans = vec![Span::styled(marker.to_string(), base_style)];

    // Highlight matches in title.
    spans.extend(highlight_spans(title, query, base_style, highlight_style));

    // Add subtitle with highlighting if present.
    if !subtitle.is_empty() {
        spans.push(Span::styled("  ".to_string(), base_style));
        let sub_style = if is_selected {
            base_style
        } else {
            Style::default().fg(colors.muted)
        };
        spans.extend(highlight_spans(subtitle, query, sub_style, highlight_style));
    }

    Line::from(spans)
}

/// Split text into spans, highlighting case-insensitive matches of `query`.
fn highlight_spans(text: &str, query: &str, base: Style, highlight: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let lower = text.to_lowercase();
    let q = query.to_lowercase();
    let mut pos = 0;

    while let Some(start) = lower[pos..].find(&q) {
        let abs_start = pos + start;
        if abs_start > pos {
            spans.push(Span::styled(text[pos..abs_start].to_string(), base));
        }
        let abs_end = abs_start + query.len();
        spans.push(Span::styled(
            text[abs_start..abs_end].to_string(),
            highlight,
        ));
        pos = abs_end;
    }

    if pos < text.len() {
        spans.push(Span::styled(text[pos..].to_string(), base));
    }

    spans
}

/// Get the style for a list item based on selection/active state.
fn item_style(is_selected: bool, is_active: bool, colors: &TuiColors) -> Style {
    if is_selected {
        Style::default().fg(colors.accent).bold()
    } else if is_active {
        Style::default().fg(colors.success)
    } else {
        Style::default()
    }
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
    if !crate::gate::tui_enabled(None) {
        print_plain(items, title);
        return Ok(None);
    }

    match run(items, title, colors)? {
        ListResult::Selected(idx) => Ok(Some(idx)),
        ListResult::Cancelled => Ok(None),
    }
}

