//! AI backend selection menu (F8).
//!
//! Opened with F8, this modal lets the user pick which AI backend
//! (Claude / Codex / Antigravity / OpenCode / Pi / Ollama variants) drives the
//! primary pane. Unlike the old cycle-on-keypress behaviour, the target is
//! shown and confirmed explicitly: F8 / ↑↓ / j k move the highlight, Enter
//! applies the selection (respawning the AI pane), Esc cancels without a switch.
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
    /// True when the menu is the initial launcher rather than an F8 switch.
    pub startup: bool,
    /// Index into [`AiBackend::all`] of the currently highlighted entry.
    pub selected: usize,
    /// Startup only: "remember this choice" checkbox. When set on confirm, the
    /// launcher is turned off (`ui.choose_backend_at_startup = false`) and later
    /// starts resume the last-used backend directly.
    pub remember: bool,
}

impl BackendSwitchState {
    /// Open the menu with the highlight on the currently active backend.
    pub fn open(&mut self, current: AiBackend) {
        self.visible = true;
        self.startup = false;
        self.remember = false;
        self.selected = AiBackend::all()
            .iter()
            .position(|b| *b == current)
            .unwrap_or(0);
    }

    /// Open as the initial launcher before any AI PTY has been spawned.
    pub fn open_startup(&mut self, suggested: AiBackend) {
        self.open(suggested);
        self.startup = true;
    }

    /// Close the menu (cancel — no switch applied).
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Toggle the "remember this choice" checkbox. Only meaningful in the
    /// startup launcher; the F8 runtime switch has no checkbox.
    pub fn toggle_remember(&mut self) {
        if self.startup {
            self.remember = !self.remember;
        }
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
        AiBackend::Claude => "Claude Code · Permission, Model, Effort, Session",
        AiBackend::OpenCode => "OpenCode CLI",
        AiBackend::Pi => "Pi coding agent CLI",
        AiBackend::Codex => "OpenAI Codex CLI",
        AiBackend::Antigravity => "Google Antigravity CLI (agy)",
        AiBackend::OllamaOpenCode => "OpenCode via Ollama launch",
        AiBackend::OllamaPi => "Pi via Ollama launch",
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

    let popup_width = area.width.saturating_sub(2).min(104);
    // title(1) + list(len) + [checkbox(1)] + footer(1) + margins(2) + borders(2)
    let checkbox_rows: u16 = if state.startup { 1 } else { 0 };
    let popup_height: u16 =
        (backends.len() as u16 + 6 + checkbox_rows).min(area.height.saturating_sub(2));

    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(if state.startup {
            " Choose AI CLI "
        } else {
            " Switch AI Backend (F8) "
        })
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(block, popup_area);

    let chunks = Layout::vertical([
        Constraint::Length(1),                     // Title
        Constraint::Length(backends.len() as u16), // Backend list
        Constraint::Length(checkbox_rows),         // Remember checkbox (startup only)
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
            Span::styled(format!("{:<14}", backend.short_label()), name_style),
            Span::styled(
                backend_description(*backend),
                Style::default().fg(Color::Gray),
            ),
        ];
        if is_active && !state.startup {
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

    if state.startup {
        let (mark, mark_style) = if state.remember {
            (
                "[x]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("[ ]", Style::default().fg(Color::DarkGray))
        };
        let checkbox = Line::from(vec![
            Span::styled(format!(" {} ", mark), mark_style),
            Span::styled(
                "Remember choice — don't ask at startup",
                Style::default().fg(Color::White),
            ),
            Span::styled(
                "  (re-enable: Settings → General)",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(checkbox), chunks[2]);
    }

    let mut footer_spans = vec![
        Span::styled(
            " F8/↑↓ ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(" Select  "),
    ];
    if state.startup {
        footer_spans.push(Span::styled(
            " Space ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ));
        footer_spans.push(Span::raw(" Remember  "));
    }
    footer_spans.extend([
        Span::styled(" Enter ", Style::default().bg(Color::Cyan).fg(Color::Black)),
        Span::raw(if state.startup {
            " Continue  "
        } else {
            " Switch  "
        }),
        Span::styled(
            " Esc ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(if state.startup {
            " Last used"
        } else {
            " Cancel"
        }),
    ]);
    frame.render_widget(Paragraph::new(Line::from(footer_spans)), chunks[3]);
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
    fn startup_mode_is_distinct_from_runtime_switch() {
        let mut state = BackendSwitchState::default();
        state.open_startup(AiBackend::Codex);
        assert!(state.startup);
        state.open(AiBackend::Claude);
        assert!(!state.startup);
    }

    #[test]
    fn next_wraps_forward() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Claude);
        assert_eq!(state.selected_backend(), AiBackend::Claude);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::Codex);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::Antigravity);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::OpenCode);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::Pi);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::OllamaOpenCode);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::OllamaPi);
        state.next();
        assert_eq!(state.selected_backend(), AiBackend::Claude);
    }

    #[test]
    fn prev_wraps_backward() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Claude);
        state.prev();
        assert_eq!(state.selected_backend(), AiBackend::OllamaPi);
        state.prev();
        assert_eq!(state.selected_backend(), AiBackend::OllamaOpenCode);
    }

    #[test]
    fn remember_toggles_only_in_the_startup_launcher() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Claude);
        state.toggle_remember();
        assert!(!state.remember, "F8 switch has no checkbox");

        state.open_startup(AiBackend::Claude);
        state.toggle_remember();
        assert!(state.remember);
        state.toggle_remember();
        assert!(!state.remember);
    }

    fn render_to_text(state: &BackendSwitchState) -> String {
        let backend = ratatui::backend::TestBackend::new(110, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), state, AiBackend::Claude))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn startup_launcher_shows_checkbox_and_full_footer() {
        let mut state = BackendSwitchState::default();
        state.open_startup(AiBackend::Claude);
        let text = render_to_text(&state);
        assert!(text.contains("[ ] Remember choice"), "{text}");
        assert!(text.contains("Space"), "{text}");
        assert!(text.contains("Last used"), "footer cut off:\n{text}");
        assert!(text.contains("OllamaPi"), "list cut off:\n{text}");

        state.toggle_remember();
        assert!(render_to_text(&state).contains("[x] Remember choice"));
    }

    #[test]
    fn f8_switch_has_no_checkbox() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Claude);
        let text = render_to_text(&state);
        assert!(!text.contains("Remember choice"), "{text}");
        assert!(text.contains("Cancel"), "{text}");
    }

    #[test]
    fn reopening_clears_a_previous_remember() {
        let mut state = BackendSwitchState::default();
        state.open_startup(AiBackend::Claude);
        state.toggle_remember();
        state.open_startup(AiBackend::Claude);
        assert!(!state.remember);
    }

    #[test]
    fn close_hides_menu() {
        let mut state = BackendSwitchState::default();
        state.open(AiBackend::Claude);
        state.close();
        assert!(!state.visible);
    }
}
