//! TUI wizard layout for `lx onboard`.
//!
//! Renders a step-driven wizard: step header, title, body text, and bottom
//! nav hints. Theme and plugin selection are handled in the CLI command via
//! the existing `show()` and `show_multi()` — this module only handles
//! informational and confirmation steps.
//!
//! # Step kinds
//! - `Info` — press enter to continue (or go back)
//! - `Confirm` — y/n prompt with a default
//! - `Done` — final screen, enter exits
//!
//! # Fallback
//! When not interactive (`gate::tui_enabled()` is false), `run_onboard_wizard`
//! renders steps as plain terminal text and prompts via stdin readline.

use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::item::TuiColors;

// ── Public types ─────────────────────────────────────────────────────────────

/// A single step in the onboarding wizard.
pub struct OnboardStep {
    /// Short title shown in the header bar (e.g. "Welcome to Lynx").
    pub title: String,
    /// Body text displayed in the content area. Supports newlines.
    pub body: String,
    /// What kind of interaction this step requires.
    pub kind: OnboardStepKind,
}

/// Interaction kind for a wizard step.
pub enum OnboardStepKind {
    /// Informational — press enter to advance, esc/b to go back.
    Info,
    /// Yes/no confirmation.
    Confirm {
        /// Prompt displayed below the body.
        prompt: String,
        /// Which answer is highlighted as the default.
        default: bool,
    },
    /// Final step — enter exits the wizard.
    Done,
}

/// Result of a single wizard step.
#[derive(Debug, Clone, PartialEq)]
pub enum OnboardResult {
    /// User confirmed (or advanced through an Info step). Bool = answer for Confirm steps.
    Confirmed(bool),
    /// User went back from this step (only possible when not the first step).
    WentBack,
    /// User quit the wizard early (q / ctrl-c).
    Quit,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Run the onboarding wizard for the given steps.
///
/// Returns one `OnboardResult` per step in order. Stops early and fills
/// remaining results with `Quit` if the user quits.
///
/// In non-interactive mode (non-TTY, agent context, etc.), falls back to
/// plain terminal output and stdin prompts.
pub fn run_onboard_wizard(
    steps: &[OnboardStep],
    colors: &TuiColors,
) -> io::Result<Vec<OnboardResult>> {
    if steps.is_empty() {
        return Ok(vec![]);
    }

    if !crate::gate::tui_enabled(lynx_config::load().ok().map(|c| c.tui.enabled)) {
        return run_plain(steps);
    }

    run_tui(steps, colors)
}

// ── TUI mode ─────────────────────────────────────────────────────────────────

fn run_tui(steps: &[OnboardStep], colors: &TuiColors) -> io::Result<Vec<OnboardResult>> {
    crate::terminal::with_terminal(|terminal| wizard_event_loop(terminal, steps, colors))
}

/// Wizard state: which step we're on and the current confirm selection.
struct WizardState {
    step: usize,
    /// For Confirm steps: which answer is currently highlighted (true=yes, false=no).
    confirm_selected: bool,
}

impl WizardState {
    fn new(first_default: bool) -> Self {
        Self { step: 0, confirm_selected: first_default }
    }
}

fn wizard_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    steps: &[OnboardStep],
    colors: &TuiColors,
) -> io::Result<Vec<OnboardResult>> {
    let first_default = match steps.first() {
        Some(OnboardStep { kind: OnboardStepKind::Confirm { default, .. }, .. }) => *default,
        _ => true,
    };
    let mut state = WizardState::new(first_default);
    let mut results: Vec<OnboardResult> = Vec::with_capacity(steps.len());

    loop {
        let current = &steps[state.step];
        terminal.draw(|f| render_step(f, steps, &state, colors))?;

        let ev = event::read()?;
        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match &current.kind {
                OnboardStepKind::Info | OnboardStepKind::Done => match key.code {
                    KeyCode::Enter => {
                        results.push(OnboardResult::Confirmed(true));
                        if state.step + 1 >= steps.len() {
                            return Ok(results);
                        }
                        state.step += 1;
                        state.confirm_selected = default_for_step(&steps[state.step]);
                    }
                    KeyCode::Char('b') | KeyCode::Esc if state.step > 0 => {
                        results.push(OnboardResult::WentBack);
                        state.step -= 1;
                        state.confirm_selected = default_for_step(&steps[state.step]);
                    }
                    KeyCode::Char('q') => {
                        results.push(OnboardResult::Quit);
                        return Ok(pad_quit(results, steps.len()));
                    }
                    _ => {}
                },

                OnboardStepKind::Confirm { .. } => match key.code {
                    KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('n') => {
                        state.confirm_selected = false;
                    }
                    KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('y') => {
                        state.confirm_selected = true;
                    }
                    KeyCode::Enter => {
                        results.push(OnboardResult::Confirmed(state.confirm_selected));
                        if state.step + 1 >= steps.len() {
                            return Ok(results);
                        }
                        state.step += 1;
                        state.confirm_selected = default_for_step(&steps[state.step]);
                    }
                    KeyCode::Char('b') | KeyCode::Esc if state.step > 0 => {
                        results.push(OnboardResult::WentBack);
                        state.step -= 1;
                        state.confirm_selected = default_for_step(&steps[state.step]);
                    }
                    KeyCode::Char('q') => {
                        results.push(OnboardResult::Quit);
                        return Ok(pad_quit(results, steps.len()));
                    }
                    _ => {}
                },
            }
        }
    }
}

fn default_for_step(step: &OnboardStep) -> bool {
    match &step.kind {
        OnboardStepKind::Confirm { default, .. } => *default,
        _ => true,
    }
}

/// Fill remaining slots with Quit after an early exit.
fn pad_quit(mut results: Vec<OnboardResult>, total: usize) -> Vec<OnboardResult> {
    while results.len() < total {
        results.push(OnboardResult::Quit);
    }
    results
}

// ── Rendering ────────────────────────────────────────────────────────────────

fn render_step(
    f: &mut Frame,
    steps: &[OnboardStep],
    state: &WizardState,
    colors: &TuiColors,
) {
    let current = &steps[state.step];
    let total = steps.len();
    let accent = colors.accent;
    let muted = colors.muted;
    let success = colors.success;

    let area = f.area();

    // Layout: header (3) | body (fill) | confirm row if needed (3) | hint (1)
    let confirm_height: u16 = match &current.kind {
        OnboardStepKind::Confirm { .. } => 3,
        _ => 0,
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(confirm_height),
            Constraint::Length(1),
        ])
        .split(area);

    // ── Header: "Step N of M — Title" ───────────────────────────────────────
    let step_label = format!(" Step {} of {} — {} ", state.step + 1, total, current.title);
    let header = Paragraph::new(step_label)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(accent)),
        )
        .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));
    f.render_widget(header, chunks[0]);

    // ── Body ─────────────────────────────────────────────────────────────────
    let body = Paragraph::new(current.body.as_str())
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT))
        .wrap(Wrap { trim: false })
        .style(Style::default());
    f.render_widget(body, chunks[1]);

    // ── Confirm row (only for Confirm steps) ────────────────────────────────
    if let OnboardStepKind::Confirm { prompt, .. } = &current.kind {
        let yes_style = if state.confirm_selected {
            Style::default().fg(success).add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(muted)
        };
        let no_style = if !state.confirm_selected {
            Style::default().fg(colors.error).add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(muted)
        };
        let confirm_line = Line::from(vec![
            Span::raw(format!(" {prompt}  ")),
            Span::styled(" Yes ", yes_style),
            Span::raw("  "),
            Span::styled(" No ", no_style),
        ]);
        let confirm = Paragraph::new(confirm_line)
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM));
        f.render_widget(confirm, chunks[2]);
    }

    // ── Bottom hint bar ──────────────────────────────────────────────────────
    let hint = match &current.kind {
        OnboardStepKind::Done => " enter=finish  q=quit ".to_string(),
        OnboardStepKind::Confirm { .. } => {
            " y/n or ←/→=select  enter=confirm  b=back  q=quit ".to_string()
        }
        OnboardStepKind::Info => {
            if state.step > 0 {
                " enter=continue  b=back  q=quit ".to_string()
            } else {
                " enter=continue  q=quit ".to_string()
            }
        }
    };
    let hint_widget = Paragraph::new(hint).style(Style::default().fg(muted));
    f.render_widget(hint_widget, chunks[3]);
}

// ── Plain fallback ────────────────────────────────────────────────────────────

fn run_plain(steps: &[OnboardStep]) -> io::Result<Vec<OnboardResult>> {
    let mut results = Vec::with_capacity(steps.len());

    for (i, step) in steps.iter().enumerate() {
        println!("\n── Step {} of {} — {} ──", i + 1, steps.len(), step.title);
        println!("{}", step.body);

        match &step.kind {
            OnboardStepKind::Info | OnboardStepKind::Done => {
                print!("Press enter to continue...");
                io::stdout().flush()?;
                let mut buf = String::new();
                io::stdin().read_line(&mut buf)?;
                results.push(OnboardResult::Confirmed(true));
            }
            OnboardStepKind::Confirm { prompt, default } => {
                let default_str = if *default { "Y/n" } else { "y/N" };
                print!("{prompt} [{default_str}]: ");
                io::stdout().flush()?;
                let mut buf = String::new();
                io::stdin().read_line(&mut buf)?;
                let answer = buf.trim().to_lowercase();
                let confirmed = match answer.as_str() {
                    "y" | "yes" => true,
                    "n" | "no" => false,
                    _ => *default, // blank → default
                };
                results.push(OnboardResult::Confirmed(confirmed));
            }
        }
    }

    Ok(results)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_quit_fills_remaining() {
        let results = vec![OnboardResult::Confirmed(true)];
        let padded = pad_quit(results, 3);
        assert_eq!(padded.len(), 3);
        assert_eq!(padded[1], OnboardResult::Quit);
        assert_eq!(padded[2], OnboardResult::Quit);
    }

    #[test]
    fn pad_quit_already_full() {
        let results = vec![
            OnboardResult::Confirmed(true),
            OnboardResult::Quit,
        ];
        let padded = pad_quit(results, 2);
        assert_eq!(padded.len(), 2);
    }

    #[test]
    fn default_for_info_step_is_true() {
        let step = OnboardStep {
            title: "t".into(),
            body: "b".into(),
            kind: OnboardStepKind::Info,
        };
        assert!(default_for_step(&step));
    }

    #[test]
    fn default_for_confirm_step_respects_field() {
        let step = OnboardStep {
            title: "t".into(),
            body: "b".into(),
            kind: OnboardStepKind::Confirm { prompt: "ok?".into(), default: false },
        };
        assert!(!default_for_step(&step));
    }

    #[test]
    fn run_onboard_wizard_empty_returns_empty() {
        let colors = TuiColors::default();
        let result = run_onboard_wizard(&[], &colors).unwrap();
        assert!(result.is_empty());
    }
}
