//! Workflow runner TUI — real-time step status + scrolling output.
//!
//! Renders a two-pane view: left shows step list with status indicators,
//! right shows live stdout/stderr output. Supports stop, background, and
//! quit actions via keybindings. Output pane is scrollable.

use std::io;
use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::item::TuiColors;
use lynx_workflow::executor::{StepStatus, StreamEvent};

// ── Public types ───────────────────────────────────────────────────────────

/// TUI-local step display phase — tracks in-flight state the executor's
/// `StepStatus` doesn't model (executor only has terminal states).
#[derive(Debug, Clone, PartialEq, Eq)]
enum TuiStepPhase {
    Pending,
    Running,
    Done(StepStatus),
}

impl TuiStepPhase {
    fn icon(&self) -> &'static str {
        match self {
            TuiStepPhase::Pending => "\u{25cb}", // ○
            TuiStepPhase::Running => "\u{25cf}", // ●
            TuiStepPhase::Done(s) => s.icon(),
        }
    }
}

/// What the user chose to do.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    phase: TuiStepPhase,
    duration_ms: Option<u64>,
}

struct TuiState {
    steps: Vec<StepState>,
    output_lines: Vec<OutputLine>,
    /// Auto-scroll to bottom of output (disabled when user scrolls up).
    auto_scroll: bool,
    /// Manual scroll offset from the top of the output.
    scroll_offset: usize,
    /// Height of the output pane's inner area (updated each render).
    output_inner_height: usize,
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
                phase: TuiStepPhase::Pending,
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
            scroll_offset: 0,
            output_inner_height: 0,
            done: false,
            success: None,
            total_duration_ms: None,
            list_state,
            filter_step: None,
        }
    }

    fn handle_event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::StepStarted { name } => {
                // Auto-advance the step list selection to the running step.
                if let Some(idx) = self.steps.iter().position(|s| s.name == name) {
                    self.list_state.select(Some(idx));
                }
                if let Some(s) = self.steps.iter_mut().find(|s| s.name == name) {
                    s.phase = TuiStepPhase::Running;
                }
                // Add a visible separator so every step is clearly visible in output.
                self.output_lines.push(OutputLine {
                    step_name: name.clone(),
                    text: String::new(),
                    is_stderr: false,
                });
                self.output_lines.push(OutputLine {
                    step_name: name,
                    text: "\u{2500}\u{2500}\u{2500} started \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string(),
                    is_stderr: false,
                });
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            StreamEvent::StepOutput {
                name,
                line,
                is_stderr,
            } => {
                self.output_lines.push(OutputLine {
                    step_name: name,
                    text: line,
                    is_stderr,
                });
                // If auto-scrolling, keep scroll at bottom.
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            StreamEvent::StepFinished {
                name,
                status,
                duration_ms,
            } => {
                // Add a completion marker in the output.
                let icon = if status.icon().is_empty() {
                    "\u{25cb}"
                } else {
                    status.icon()
                };
                self.output_lines.push(OutputLine {
                    step_name: name.clone(),
                    text: format!("{icon} finished ({duration_ms}ms)"),
                    is_stderr: false,
                });
                if let Some(s) = self.steps.iter_mut().find(|s| s.name == name) {
                    s.phase = TuiStepPhase::Done(status);
                    s.duration_ms = Some(duration_ms);
                }
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
            StreamEvent::Done {
                success,
                duration_ms,
            } => {
                self.done = true;
                self.success = Some(success);
                self.total_duration_ms = Some(duration_ms);

                // Append a final summary so the user always sees all step results.
                self.output_lines.push(OutputLine {
                    step_name: "workflow".to_string(),
                    text: String::new(),
                    is_stderr: false,
                });
                self.output_lines.push(OutputLine {
                    step_name: "workflow".to_string(),
                    text: "\u{2500}\u{2500}\u{2500} summary \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string(),
                    is_stderr: false,
                });
                for s in &self.steps {
                    let icon = s.phase.icon();
                    let dur = s
                        .duration_ms
                        .map(|ms| format!(" ({ms}ms)"))
                        .unwrap_or_default();
                    self.output_lines.push(OutputLine {
                        step_name: "workflow".to_string(),
                        text: format!("  {icon} {}{dur}", s.name),
                        is_stderr: s.phase == TuiStepPhase::Done(StepStatus::Failed),
                    });
                }
                let total_icon = if success { "\u{2713}" } else { "\u{2717}" };
                self.output_lines.push(OutputLine {
                    step_name: "workflow".to_string(),
                    text: format!("  {total_icon} total: {duration_ms}ms"),
                    is_stderr: !success,
                });

                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
            }
        }
    }

    fn visible_output_count(&self) -> usize {
        match &self.filter_step {
            Some(name) => self
                .output_lines
                .iter()
                .filter(|l| &l.step_name == name)
                .count(),
            None => self.output_lines.len(),
        }
    }

    fn visible_output(&self) -> Vec<&OutputLine> {
        match &self.filter_step {
            Some(name) => self
                .output_lines
                .iter()
                .filter(|l| &l.step_name == name)
                .collect(),
            None => self.output_lines.iter().collect(),
        }
    }

    fn max_scroll(&self) -> usize {
        self.visible_output_count()
            .saturating_sub(self.output_inner_height)
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll();
    }

    fn scroll_up(&mut self, lines: usize) {
        self.auto_scroll = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = (self.scroll_offset + lines).min(self.max_scroll());
        // Re-enable auto-scroll if we're at the bottom.
        if self.scroll_offset >= self.max_scroll() {
            self.auto_scroll = true;
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
    rx: mpsc::Receiver<StreamEvent>,
    colors: &TuiColors,
) -> io::Result<WorkflowAction> {
    crate::terminal::with_terminal(|terminal| {
        event_loop(terminal, workflow_name, step_names, rx, colors)
    })
}

/// Check if we should use the workflow TUI.
///
/// Delegates to the shared gate — see `crate::gate::tui_enabled` for full check list.
/// Loads `[tui] enabled` from config (best-effort) to honour the user's opt-out flag.
pub fn should_use_tui() -> bool {
    let config_tui = lynx_config::load().ok().map(|c| c.tui.enabled);
    crate::gate::tui_enabled(config_tui)
}

// ── Event loop ─────────────────────────────────────────────────────────────

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    workflow_name: &str,
    step_names: &[String],
    rx: mpsc::Receiver<StreamEvent>,
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

        // Poll keyboard/mouse with short timeout so we keep refreshing output.
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
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
                        // Step list navigation.
                        KeyCode::Down | KeyCode::Char('j') => {
                            let max = state.steps.len().saturating_sub(1);
                            let cur = state.list_state.selected().unwrap_or(0);
                            state.list_state.select(Some((cur + 1).min(max)));
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let cur = state.list_state.selected().unwrap_or(0);
                            state.list_state.select(Some(cur.saturating_sub(1)));
                        }
                        // Output scrolling.
                        KeyCode::PageUp => state.scroll_up(state.output_inner_height),
                        KeyCode::PageDown => state.scroll_down(state.output_inner_height),
                        KeyCode::Home => {
                            state.auto_scroll = false;
                            state.scroll_offset = 0;
                        }
                        KeyCode::End => {
                            state.auto_scroll = true;
                            state.scroll_to_bottom();
                        }
                        // Shift+Up/Down for output scroll by single line.
                        KeyCode::Char('K') => state.scroll_up(1),
                        KeyCode::Char('J') => state.scroll_down(1),
                        KeyCode::Enter => {
                            // Toggle filter: show only selected step's output.
                            if let Some(idx) = state.list_state.selected() {
                                if let Some(step) = state.steps.get(idx) {
                                    if state.filter_step.as_ref() == Some(&step.name) {
                                        state.filter_step = None;
                                    } else {
                                        state.filter_step = Some(step.name.clone());
                                    }
                                    // Reset scroll when changing filter.
                                    state.auto_scroll = true;
                                    state.scroll_to_bottom();
                                }
                            }
                        }
                        KeyCode::Char('a') => {
                            state.filter_step = None;
                            state.auto_scroll = true;
                            state.scroll_to_bottom();
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => state.scroll_up(3),
                    MouseEventKind::ScrollDown => state.scroll_down(3),
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

// ── Rendering ──────────────────────────────────────────────────────────────

fn render(frame: &mut Frame, workflow_name: &str, colors: &TuiColors, state: &mut TuiState) {
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

            let icon_color = match &step.phase {
                TuiStepPhase::Pending => colors.muted,
                TuiStepPhase::Running => colors.accent,
                TuiStepPhase::Done(StepStatus::Passed) => colors.success,
                TuiStepPhase::Done(StepStatus::Failed) => colors.error,
                TuiStepPhase::Done(StepStatus::Skipped) => colors.muted,
                TuiStepPhase::Done(StepStatus::TimedOut) => colors.warning,
            };
            let icon = step.phase.icon();

            let duration = step
                .duration_ms
                .map(|ms| format!(" {ms}ms"))
                .unwrap_or_default();

            // Name color reflects status; selected gets bold.
            let status_color = match &step.phase {
                TuiStepPhase::Pending => None, // default terminal color
                TuiStepPhase::Running => Some(colors.accent),
                TuiStepPhase::Done(StepStatus::Passed) => Some(colors.success),
                TuiStepPhase::Done(StepStatus::Failed) => Some(colors.error),
                TuiStepPhase::Done(StepStatus::Skipped) => Some(colors.muted),
                TuiStepPhase::Done(StepStatus::TimedOut) => Some(colors.warning),
            };

            let name_style = if is_selected {
                let base = Style::default().bold();
                match status_color {
                    Some(c) => base.fg(c),
                    None => base.fg(colors.accent),
                }
            } else if is_filtered {
                Style::default().fg(colors.accent)
            } else {
                match status_color {
                    Some(c) => Style::default().fg(c),
                    None => Style::default(),
                }
            };

            let line = Line::from(vec![
                Span::styled(format!(" {icon} "), Style::default().fg(icon_color)),
                Span::styled(step.name.clone(), name_style),
                Span::styled(duration, Style::default().fg(colors.muted)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let completed = state
        .steps
        .iter()
        .filter(|s| matches!(s.phase, TuiStepPhase::Done(_)))
        .count();

    let title = format!(" {workflow_name} ({completed}/{}) ", state.steps.len());

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

fn render_output(frame: &mut Frame, area: Rect, colors: &TuiColors, state: &mut TuiState) {
    // Update inner height for scroll calculations (before borrowing state).
    let inner_height = area.height.saturating_sub(2) as usize;
    state.output_inner_height = inner_height;

    let total_lines = state.visible_output_count();

    // Clamp scroll offset. Auto-scroll pins to bottom.
    let max_scroll = total_lines.saturating_sub(inner_height);
    if state.auto_scroll || state.scroll_offset > max_scroll {
        state.scroll_offset = max_scroll;
    }

    let visible = state.visible_output();
    let lines: Vec<Line> = visible
        .iter()
        .map(|line| {
            let prefix_style = Style::default().fg(colors.muted);

            // Marker lines (started/finished) render in accent color.
            let is_marker = line.text.contains("\u{2500}\u{2500}\u{2500}")
                || line.text.starts_with('\u{2713}')
                || line.text.starts_with('\u{2717}')
                || line.text.starts_with('\u{2014}')
                || line.text.starts_with('\u{23f0}');

            let text_style = if is_marker {
                Style::default().fg(colors.accent)
            } else if line.is_stderr {
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

    // Title shows filter state + scroll position.
    let scroll_indicator = if !state.auto_scroll && total_lines > inner_height {
        let pct = if max_scroll > 0 {
            (state.scroll_offset * 100) / max_scroll
        } else {
            100
        };
        format!(" {pct}%")
    } else {
        String::new()
    };

    let title = match &state.filter_step {
        Some(name) => format!(" {name}{scroll_indicator} (a=all) "),
        None => format!(" Output{scroll_indicator} "),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.muted))
        .title(title)
        .title_style(Style::default().fg(colors.muted));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((state.scroll_offset as u16, 0));

    frame.render_widget(paragraph, area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, colors: &TuiColors, state: &TuiState) {
    let keys = if state.done {
        let status = if state.success.unwrap_or(false) {
            Span::styled(
                " \u{2713} Done ",
                Style::default().fg(colors.success).bold(),
            )
        } else {
            Span::styled(
                " \u{2717} Failed ",
                Style::default().fg(colors.error).bold(),
            )
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
            Span::styled("  PgUp/Dn", Style::default().fg(colors.accent).bold()),
            Span::styled(" scroll", Style::default().fg(colors.muted)),
        ])
    } else {
        let running = state
            .steps
            .iter()
            .filter(|s| s.phase == TuiStepPhase::Running)
            .count();

        Line::from(vec![
            Span::styled(
                format!(" \u{25cf} Running ({running} active) "),
                Style::default().fg(colors.accent).bold(),
            ),
            Span::styled(" s", Style::default().fg(colors.accent).bold()),
            Span::styled(" stop", Style::default().fg(colors.muted)),
            Span::styled("  b", Style::default().fg(colors.accent).bold()),
            Span::styled(" bg", Style::default().fg(colors.muted)),
            Span::styled("  q", Style::default().fg(colors.accent).bold()),
            Span::styled(" quit", Style::default().fg(colors.muted)),
            Span::styled("  J/K", Style::default().fg(colors.accent).bold()),
            Span::styled(" scroll", Style::default().fg(colors.muted)),
        ])
    };

    frame.render_widget(Paragraph::new(keys), area);
}
