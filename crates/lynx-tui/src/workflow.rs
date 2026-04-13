//! Workflow runner TUI — real-time step status + scrolling output.
//!
//! Renders a two-pane view: left shows step list with status indicators,
//! right shows live stdout/stderr output. Supports stop, background, and
//! quit actions via keybindings.

use std::io;
use std::sync::mpsc;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::item::TuiColors;

// ── Public types ───────────────────────────────────────────────────────────

/// Events sent from the executor to the TUI.
#[derive(Debug, Clone)]
pub enum WorkflowEvent {
    /// A step has started executing.
    StepStarted { name: String },
    /// A line of output from a step.
    StepOutput { name: String, line: String, is_stderr: bool },
    /// A step has finished.
    StepFinished { name: String, status: StepStepStatus, duration_ms: u64 },
    /// The entire workflow is done.
    Done { success: bool, duration_ms: u64 },
}

/// Step status for the TUI (mirrors executor::StepStatus without coupling).
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowStepStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Skipped,
    TimedOut,
}

// Keep a short alias for the enum used in events.
pub type StepStepStatus = WorkflowStepStatus;

/// What the user chose to do.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowAction {
    /// Workflow finished, user pressed q.
    Completed,
    /// User pressed b to background.
    Background,
    /// User pressed s or ctrl+c to stop.
    Stopped,
}

// ── Internal state ─────────────────────────────────────────────────────────

struct StepState {
    name: String,
    status: WorkflowStepStatus,
    duration_ms: Option<u64>,
}

struct TuiState {
    steps: Vec<StepState>,
    output_lines: Vec<OutputLine>,
    /// Auto-scroll to bottom of output.
    auto_scroll: bool,
    /// Whether the workflow has finished.
    done: bool,
    /// Overall success (set when Done event arrives).
    success: Option<bool>,
    /// Total duration (set when Done event arrives).
    total_duration_ms: Option<u64>,
    /// Currently highlighted step in the list.
    list_state: ListState,
    /// Filter output to a specific step (None = all).
    filter_step: Option<String>,
}

struct OutputLine {
    step_name: String,
    text: String,
    is_stderr: bool,
}

impl TuiState {
    fn new(step_names: &[String]) -> Self {
        let steps = step_names
            .iter()
            .map(|name| StepState {
                name: name.clone(),
                status: WorkflowStepStatus::Pending,
                duration_ms: None,
            })
            .collect();
        let mut list_state = ListState::default();
        if !step_names.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            steps,
            output_lines: Vec::new(),
            auto_scroll: true,
            done: false,
            success: None,
            total_duration_ms: None,
            list_state,
            filter_step: None,
        }
    }

    fn handle_event(&mut self, event: WorkflowEvent) {
        match event {
            WorkflowEvent::StepStarted { name } => {
                if let Some(s) = self.steps.iter_mut().find(|s| s.name == name) {
                    s.status = WorkflowStepStatus::Running;
                }
            }
            WorkflowEvent::StepOutput { name, line, is_stderr } => {
                self.output_lines.push(OutputLine {
                    step_name: name,
                    text: line,
                    is_stderr,
                });
            }
            WorkflowEvent::StepFinished { name, status, duration_ms } => {
                if let Some(s) = self.steps.iter_mut().find(|s| s.name == name) {
                    s.status = status;
                    s.duration_ms = Some(duration_ms);
                }
            }
            WorkflowEvent::Done { success, duration_ms } => {
                self.done = true;
                self.success = Some(success);
                self.total_duration_ms = Some(duration_ms);
            }
        }
    }

    fn visible_output(&self) -> Vec<&OutputLine> {
        match &self.filter_step {
            Some(name) => self.output_lines.iter().filter(|l| &l.step_name == name).collect(),
            None => self.output_lines.iter().collect(),
        }
    }
}

// ── Public entry point ─────────────────────────────────────────────────────

/// Run the workflow TUI. Blocks until the user exits.
///
/// `step_names` is the ordered list of step names from the workflow.
/// `rx` receives events from the executor running on another thread/task.
/// Returns the action the user took.
pub fn run_workflow_tui(
    workflow_name: &str,
    step_names: &[String],
    rx: mpsc::Receiver<WorkflowEvent>,
    colors: &TuiColors,
) -> io::Result<WorkflowAction> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        event_loop(&mut terminal, workflow_name, step_names, rx, colors)
    }));

    terminal::disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    match result {
        Ok(inner) => inner,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// Check if we should use the workflow TUI (same logic as list TUI).
pub fn should_use_tui() -> bool {
    use crossterm::tty::IsTty;
    if !io::stdout().is_tty() {
        return false;
    }
    if let Ok(ctx) = std::env::var("LYNX_CONTEXT") {
        if ctx == "agent" {
            return false;
        }
    }
    true
}

// ── Event loop ─────────────────────────────────────────────────────────────

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    workflow_name: &str,
    step_names: &[String],
    rx: mpsc::Receiver<WorkflowEvent>,
    colors: &TuiColors,
) -> io::Result<WorkflowAction> {
    let mut state = TuiState::new(step_names);

    loop {
        // Drain all pending workflow events (non-blocking).
        while let Ok(ev) = rx.try_recv() {
            state.handle_event(ev);
        }

        // Render.
        terminal.draw(|frame| {
            render(frame, workflow_name, colors, &mut state);
        })?;

        // Poll keyboard with short timeout so we keep refreshing output.
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Ctrl+C always stops.
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    return Ok(WorkflowAction::Stopped);
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if state.done {
                            return Ok(WorkflowAction::Completed);
                        }
                        // While running, q also stops.
                        return Ok(WorkflowAction::Stopped);
                    }
                    KeyCode::Char('b') => {
                        if !state.done {
                            return Ok(WorkflowAction::Background);
                        }
                    }
                    KeyCode::Char('s') => {
                        if !state.done {
                            return Ok(WorkflowAction::Stopped);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max = state.steps.len().saturating_sub(1);
                        let cur = state.list_state.selected().unwrap_or(0);
                        state.list_state.select(Some((cur + 1).min(max)));
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let cur = state.list_state.selected().unwrap_or(0);
                        state.list_state.select(Some(cur.saturating_sub(1)));
                    }
                    KeyCode::Enter => {
                        // Toggle filter: show only selected step's output.
                        if let Some(idx) = state.list_state.selected() {
                            if let Some(step) = state.steps.get(idx) {
                                if state.filter_step.as_ref() == Some(&step.name) {
                                    state.filter_step = None;
                                } else {
                                    state.filter_step = Some(step.name.clone());
                                }
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        // Show all output (clear filter).
                        state.filter_step = None;
                    }
                    _ => {}
                }
            }
        }
    }
}

// ── Rendering ──────────────────────────────────────────────────────────────

fn render(
    frame: &mut Frame,
    workflow_name: &str,
    colors: &TuiColors,
    state: &mut TuiState,
) {
    let area = frame.area();

    // Layout: top (main content) + bottom (status bar).
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    // Main: left (steps) + right (output).
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(36), Constraint::Min(1)])
        .split(outer[0]);

    render_steps(frame, main[0], workflow_name, colors, state);
    render_output(frame, main[1], colors, state);
    render_status_bar(frame, outer[1], colors, state);
}

fn render_steps(
    frame: &mut Frame,
    area: Rect,
    workflow_name: &str,
    colors: &TuiColors,
    state: &mut TuiState,
) {
    let items: Vec<ListItem> = state
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let is_selected = state.list_state.selected() == Some(i);
            let is_filtered = state.filter_step.as_ref() == Some(&step.name);

            let (icon, icon_color) = match step.status {
                WorkflowStepStatus::Pending => ("\u{25cb}", colors.muted),   // ○
                WorkflowStepStatus::Running => ("\u{25cf}", colors.accent),  // ●
                WorkflowStepStatus::Passed => ("\u{2713}", colors.success),  // ✓
                WorkflowStepStatus::Failed => ("\u{2717}", colors.error),    // ✗
                WorkflowStepStatus::Skipped => ("\u{2014}", colors.muted),   // —
                WorkflowStepStatus::TimedOut => ("\u{23f0}", colors.warning), // ⏰
            };

            let duration = step
                .duration_ms
                .map(|ms| format!(" {ms}ms"))
                .unwrap_or_default();

            let name_style = if is_selected {
                Style::default().fg(colors.accent).bold()
            } else if is_filtered {
                Style::default().fg(colors.accent)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(format!(" {icon} "), Style::default().fg(icon_color)),
                Span::styled(step.name.clone(), name_style),
                Span::styled(duration, Style::default().fg(colors.muted)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let title = format!(" {workflow_name} ({}/{}) ",
        state.steps.iter().filter(|s| matches!(s.status, WorkflowStepStatus::Passed | WorkflowStepStatus::Failed | WorkflowStepStatus::Skipped | WorkflowStepStatus::TimedOut)).count(),
        state.steps.len(),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.muted))
        .title(title)
        .title_style(Style::default().fg(colors.accent).bold());

    let list = List::new(items)
        .block(block)
        .highlight_symbol("")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Never);

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

fn render_output(
    frame: &mut Frame,
    area: Rect,
    colors: &TuiColors,
    state: &TuiState,
) {
    let visible = state.visible_output();

    let lines: Vec<Line> = visible
        .iter()
        .map(|line| {
            let prefix_style = Style::default().fg(colors.muted);
            let text_style = if line.is_stderr {
                Style::default().fg(colors.warning)
            } else {
                Style::default()
            };

            Line::from(vec![
                Span::styled(format!("[{}] ", line.step_name), prefix_style),
                Span::styled(line.text.clone(), text_style),
            ])
        })
        .collect();

    let title = match &state.filter_step {
        Some(name) => format!(" Output: {name} (a=all) "),
        None => " Output (enter=filter step) ".to_string(),
    };

    // Auto-scroll: calculate scroll position to show the bottom.
    let inner_height = area.height.saturating_sub(2) as usize; // minus borders
    let total_lines = lines.len();
    let scroll = if state.auto_scroll && total_lines > inner_height {
        (total_lines - inner_height) as u16
    } else {
        0
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.muted))
        .title(title)
        .title_style(Style::default().fg(colors.muted));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);
}

fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    colors: &TuiColors,
    state: &TuiState,
) {
    let keys = if state.done {
        let status = if state.success.unwrap_or(false) {
            Span::styled(" \u{2713} Done ", Style::default().fg(colors.success).bold())
        } else {
            Span::styled(" \u{2717} Failed ", Style::default().fg(colors.error).bold())
        };

        let dur = state
            .total_duration_ms
            .map(|ms| format!("({ms}ms) "))
            .unwrap_or_default();

        Line::from(vec![
            status,
            Span::styled(dur, Style::default().fg(colors.muted)),
            Span::styled("  q", Style::default().fg(colors.accent).bold()),
            Span::styled(" quit", Style::default().fg(colors.muted)),
        ])
    } else {
        let running = state
            .steps
            .iter()
            .filter(|s| s.status == WorkflowStepStatus::Running)
            .count();

        Line::from(vec![
            Span::styled(
                format!(" \u{25cf} Running ({running} active) "),
                Style::default().fg(colors.accent).bold(),
            ),
            Span::styled(" s", Style::default().fg(colors.accent).bold()),
            Span::styled(" stop", Style::default().fg(colors.muted)),
            Span::styled("  b", Style::default().fg(colors.accent).bold()),
            Span::styled(" background", Style::default().fg(colors.muted)),
            Span::styled("  q", Style::default().fg(colors.accent).bold()),
            Span::styled(" quit", Style::default().fg(colors.muted)),
        ])
    };

    frame.render_widget(Paragraph::new(keys), area);
}
