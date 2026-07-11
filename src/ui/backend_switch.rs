//! AI backend selection menu (F8).
//!
//! Opened with F8, this modal lets the user pick which AI backend
//! (Claude / OpenCode / Pi) drives the primary pane. Unlike the old
//! cycle-on-keypress behaviour, the target is shown and confirmed
//! explicitly: F8 / ↑↓ / j k move the highlight, Enter applies the
//! selection (respawning the AI pane), Esc cancels without a switch.
//!
//! State follows the same `visible + selected: usize` pattern as
//! [`crate::ui::permission_mode::PermissionModeState`].

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::backend::AiBackend;

/// State for the F8 backend-selection menu.
#[derive(Debug, Clone, Default)]
pub struct BackendSwitchState {
    pub visible: bool,
    /// Index into [`AiBackend::all`] of the currently highlighted entry.
    pub selected: usize,
}

impl BackendSwitchState {
    /// Open the menu with the highlight on the currently active backend.
    pub fn open(&mut self, current: AiBackend) {
        self.visible = true;
        self.selected = AiBackend::all()
            .iter()
            .position(|b| *b == current)
            .unwrap_or(0);
    }

    /// Close the menu (cancel — no switch applied).
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Move the highlight to the next entry (wraps). Drives F8 / ↓ / j.
    pub fn next(&mut self) {
        let len = AiBackend::all().len();
        self.selected = (self.selected + 1) % len;
    }

    /// Move the highlight to the previous entry (wraps). Drives ↑ / k.
    pub fn prev(&mut self) {
        let len = AiBackend::all().len();
        self.selected = (self.selected + len - 1) % len;
    }

    /// The backend currently under the highlight.
    pub fn selected_backend(&self) -> AiBackend {
        AiBackend::all()
            .get(self.selected)
            .copied()
            .unwrap_or_default()
    }
}

/// Short, human-readable description shown next to each backend.
fn backend_description(backend: AiBackend) -> &'static str {
    match backend {
        AiBackend::Claude => "Anthropic Claude Code (permission/model/effort flags)",
        AiBackend::OpenCode => "OpenCode CLI",
        AiBackend::Pi => "Pi CLI (by Inflection)",
    }
}

// ─── Render ──────────────────────────────────────────────────────────────

/// Render the backend-selection menu. `active` is the backend currently
/// driving the AI pane (marked "← active"); `state.selected` is the highlight.
pub fn render(frame: &mut Frame, area: Rect, state: &BackendSwitchState, active: AiBackend) {
    if !state.visible {
        return;
    }

    let backends = AiBackend::all();

    let popup_width: u16 = 60;
    // title(2) + list(len) + footer(2) + borders(2)
    let popup_height: u16 = (backends.len() as u16 + 6).min(area.height.saturating_sub(2));

    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Switch AI Backend (F8) ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(block, popup_area);

    let chunks = Layout::vertical([
        Constraint::Length(1),                     // Title
        Constraint::Length(backends.len() as u16), // Backend list
        Constraint::Min(1),                        // Footer
    ])
    .split(popup_area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    }));

    let title = Paragraph::new("Select the AI backend for the primary pane:")
        .style(Style::default().fg(Color::White));
    frame.render_widget(title, chunks[0]);

    let mut items: Vec<ListItem> = Vec::new();
    for (i, backend) in backends.iter().enumerate() {
        let is_selected = i == state.selected;
        let is_active = *backend == active;

        let marker = if is_selected { "(•)" } else { "( )" };
        let marker_style = Style::default().fg(if is_selected {
            Color::Yellow
        } else {
            Color::DarkGray
        });

        let name_style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let mut spans = vec![
            Span::styled(format!(" {} ", marker), marker_style),
            Span::styled(format!("{:<9}", backend.short_label()), name_style),
            Span::styled(
                backend_description(*backend),
                Style::default().fg(Color::Gray),
            ),
        ];
        if is_active {
            spans.push(Span::styled(
                "  ← active",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        items.push(ListItem::new(Line::from(spans)));
    }
    frame.render_widget(List::new(items), chunks[1]);

    let footer = Line::from(vec![
        Span::styled(
            " F8/↑↓ ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(" Select  "),
        Span::styled(" Enter ", Style::default().bg(Color::Cyan).fg(Color::Black)),
        Span::raw(" Switch  "),
        Span::styled(
            " Esc ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(" Cancel"),
    ]);
    frame.render_widget(Paragraph::new(footer), chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_highlights_active_backend() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Pi);
        assert!(state.visible);
        assert_eq!(state.selected_backend(), AiBackend::Pi);

        state.open(AiBackend::OpenCode);
        assert_eq!(state.selected_backend(), AiBackend::OpenCode);
    }

    #[test]
    fn next_wraps_forward() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Claude);
        assert_eq!(state.selected_backend(), AiBackend::Claude);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::OpenCode);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::Pi);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::Claude);
    }

    #[test]
    fn prev_wraps_backward() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Claude);
        state.prev();
        assert_eq!(state.selected_backend(), AiBackend::Pi);
        state.prev();
        assert_eq!(state.selected_backend(), AiBackend::OpenCode);
    }

    #[test]
    fn close_hides_menu() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Claude);
        state.close();
        assert!(!state.visible);
    }
}
