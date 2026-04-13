//! Preview pane rendering for the interactive list.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::item::{ListItem, TuiColors};
use crate::list::AppState;

/// Render the preview pane for the currently selected item.
pub fn render_preview<T: ListItem>(
    frame: &mut Frame,
    area: Rect,
    items: &[T],
    colors: &TuiColors,
    state: &AppState,
) {
    let content = match state.selected_original_index() {
        Some(idx) => {
            let item = &items[idx];
            let detail = item.detail();
            if detail.is_empty() {
                build_fallback_preview(item, area, colors)
            } else {
                build_detail_preview(item, &detail, area, colors)
            }
        }
        None => {
            let msg = if state.query.is_empty() {
                "No items"
            } else {
                "No matches"
            };
            Text::from(Line::from(Span::styled(
                msg,
                Style::default().fg(colors.error),
            )))
        }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.muted))
        .title(" Preview ")
        .title_style(Style::default().fg(colors.muted));

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Fallback preview when item has no detail(): show title + subtitle + category.
fn build_fallback_preview<T: ListItem>(item: &T, _area: Rect, colors: &TuiColors) -> Text<'static> {
    let mut lines = vec![Line::from(Span::styled(
        item.title().to_string(),
        Style::default().fg(colors.accent).bold(),
    ))];
    let sub = item.subtitle();
    if !sub.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(sub));
    }
    if let Some(cat) = item.category() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Type: ", Style::default().fg(colors.muted)),
            Span::raw(cat.to_string()),
        ]));
    }
    Text::from(lines)
}

/// Full detail preview: title header + separator + detail body.
fn build_detail_preview<T: ListItem>(
    item: &T,
    detail: &str,
    area: Rect,
    colors: &TuiColors,
) -> Text<'static> {
    let mut lines = vec![
        Line::from(Span::styled(
            item.title().to_string(),
            Style::default().fg(colors.accent).bold(),
        )),
        Line::from(Span::styled(
            "─".repeat((area.width as usize).saturating_sub(4)),
            Style::default().fg(colors.muted),
        )),
    ];
    for line in detail.lines() {
        lines.push(Line::from(line.to_string()));
    }
    Text::from(lines)
}
