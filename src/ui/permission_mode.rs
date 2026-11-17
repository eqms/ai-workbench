use std::time::{Duration, Instant};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::pty::StartupOptions;
use crate::types::{ClaudeEffort, ClaudeModel, ClaudePermissionMode};

/// Sections inside the Claude startup dialog. Navigated via Tab / Shift+Tab
/// or by clicking an option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogSection {
    #[default]
    Permission,
    Model,
    Effort,
}

impl DialogSection {
    fn next(self) -> Self {
        match self {
            Self::Permission => Self::Model,
            Self::Model => Self::Effort,
            Self::Effort => Self::Permission,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Permission => Self::Effort,
            Self::Model => Self::Permission,
            Self::Effort => Self::Model,
        }
    }
}

/// What a mouse click inside the startup dialog hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogClick {
    Permission(usize),
    Model(usize),
    Effort(usize),
    Confirm,
    Cancel,
    /// Inside the dialog but on no control, or outside it.
    None,
}

/// Screen areas of the clickable controls, recorded by the last render.
#[derive(Debug, Clone, Default)]
struct HitAreas {
    permission_rows: Vec<Rect>,
    model_items: Vec<Rect>,
    effort_items: Vec<Rect>,
    confirm: Rect,
    cancel: Rect,
}

/// State of the Claude startup dialog: permission mode, model and effort.
#[derive(Debug, Clone, Default)]
pub struct PermissionModeState {
    pub visible: bool,
    pub confirmed: bool,
    pub section: DialogSection,

    pub permission_selected: usize,
    pub model_selected: usize,
    pub effort_selected: usize,

    /// Name of the project the choices are remembered for (dialog title).
    pub project_label: String,

    hit_areas: HitAreas,
    /// Last click on a permission row, to recognise a double click.
    last_permission_click: Option<(usize, Instant)>,
}

/// Time window in which a second click on the same permission row counts as
/// a double click (same default as the file browser).
const DOUBLE_CLICK: Duration = Duration::from_millis(300);

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

impl PermissionModeState {
    /// Open the dialog preselected with `defaults` — the values remembered for
    /// the project named `project_label`.
    pub(crate) fn open_with_defaults(&mut self, defaults: StartupOptions, project_label: String) {
        self.visible = true;
        self.confirmed = false;
        self.section = DialogSection::Permission;
        self.project_label = project_label;
        self.last_permission_click = None;

        self.permission_selected = ClaudePermissionMode::all()
            .iter()
            .position(|m| *m == defaults.permission_mode)
            .unwrap_or(0);
        self.model_selected = ClaudeModel::all()
            .iter()
            .position(|m| *m == defaults.model)
            .unwrap_or(0);
        self.effort_selected = ClaudeEffort::all()
            .iter()
            .position(|e| *e == defaults.effort)
            .unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.confirmed = false;
    }

    pub fn confirm(&mut self) {
        self.confirmed = true;
        self.visible = false;
    }

    // ── Section navigation ────────────────────────────────────────────────

    pub fn next_section(&mut self) {
        self.section = self.section.next();
    }

    pub fn prev_section(&mut self) {
        self.section = self.section.prev();
    }

    // ── Intra-section navigation (↑↓ for lists, ←→ for radios) ───────────

    pub fn prev_item(&mut self) {
        match self.section {
            DialogSection::Permission if self.permission_selected > 0 => {
                self.permission_selected -= 1;
            }
            DialogSection::Model if self.model_selected > 0 => {
                self.model_selected -= 1;
            }
            DialogSection::Effort if self.effort_selected > 0 => {
                self.effort_selected -= 1;
            }
            _ => {}
        }
    }

    pub fn next_item(&mut self) {
        match self.section {
            DialogSection::Permission
                if self.permission_selected + 1 < ClaudePermissionMode::all().len() =>
            {
                self.permission_selected += 1;
            }
            DialogSection::Model if self.model_selected + 1 < ClaudeModel::all().len() => {
                self.model_selected += 1;
            }
            DialogSection::Effort if self.effort_selected + 1 < ClaudeEffort::all().len() => {
                self.effort_selected += 1;
            }
            _ => {}
        }
    }

    // ── Mouse ─────────────────────────────────────────────────────────────

    /// Which control lies under `(x, y)`, based on the last render.
    pub fn hit_test(&self, x: u16, y: u16) -> DialogClick {
        let areas = &self.hit_areas;
        if contains(areas.confirm, x, y) {
            return DialogClick::Confirm;
        }
        if contains(areas.cancel, x, y) {
            return DialogClick::Cancel;
        }
        let find = |rects: &[Rect]| rects.iter().position(|r| contains(*r, x, y));
        if let Some(i) = find(&areas.permission_rows) {
            return DialogClick::Permission(i);
        }
        if let Some(i) = find(&areas.model_items) {
            return DialogClick::Model(i);
        }
        if let Some(i) = find(&areas.effort_items) {
            return DialogClick::Effort(i);
        }
        DialogClick::None
    }

    /// Select a permission mode by click. Returns `true` when this click is
    /// the second one on the same row within the double-click window — the
    /// caller then starts Claude right away.
    pub fn click_permission(&mut self, index: usize) -> bool {
        self.click_permission_at(index, Instant::now())
    }

    fn click_permission_at(&mut self, index: usize, now: Instant) -> bool {
        if index >= ClaudePermissionMode::all().len() {
            return false;
        }
        self.section = DialogSection::Permission;
        self.permission_selected = index;
        let double = matches!(
            self.last_permission_click,
            Some((last, at)) if last == index && now.duration_since(at) <= DOUBLE_CLICK
        );
        self.last_permission_click = if double { None } else { Some((index, now)) };
        double
    }

    pub fn select_model(&mut self, index: usize) {
        if index < ClaudeModel::all().len() {
            self.section = DialogSection::Model;
            self.model_selected = index;
        }
    }

    pub fn select_effort(&mut self, index: usize) {
        if index < ClaudeEffort::all().len() {
            self.section = DialogSection::Effort;
            self.effort_selected = index;
        }
    }

    // ── Accessors ────────────────────────────────────────────────────────

    pub fn selected_permission_mode(&self) -> ClaudePermissionMode {
        ClaudePermissionMode::all()
            .get(self.permission_selected)
            .copied()
            .unwrap_or_default()
    }

    pub fn selected_model(&self) -> ClaudeModel {
        ClaudeModel::all()
            .get(self.model_selected)
            .copied()
            .unwrap_or_default()
    }

    pub fn selected_effort(&self) -> ClaudeEffort {
        ClaudeEffort::all()
            .get(self.effort_selected)
            .copied()
            .unwrap_or_default()
    }
}

// ─── Render ──────────────────────────────────────────────────────────────

/// Width of a radio item as rendered: `prefix` + marker `(•)` + space + name.
fn radio_width(prefix: &str, name: &str) -> u16 {
    (prefix.chars().count() + 3 + 1 + name.chars().count()) as u16
}

/// Horizontal areas of consecutive items of the given widths in `row`,
/// clipped to the row.
fn item_rects(row: Rect, widths: impl Iterator<Item = u16>) -> Vec<Rect> {
    let right = row.x + row.width;
    let mut x = row.x;
    widths
        .map(|width| {
            let start = x.min(right);
            let end = x.saturating_add(width).min(right);
            x = x.saturating_add(width);
            Rect::new(start, row.y, end - start, row.height)
        })
        .collect()
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut PermissionModeState) {
    if !state.visible {
        return;
    }

    let modes = ClaudePermissionMode::all();
    let models = ClaudeModel::all();
    let efforts = ClaudeEffort::all();

    let popup_width: u16 = 80.min(area.width);
    // borders(2) + padding(2) + title(2) + permission(len+1) + model(2)
    // + effort(2) + spacer(1) + footer(1)
    let popup_height: u16 = (modes.len() as u16 + 13).min(area.height.saturating_sub(2));

    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let title = if state.project_label.is_empty() {
        " Claude Code Startup ".to_string()
    } else {
        format!(" Claude Code Startup — {} ", state.project_label)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title)
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(block, popup_area);

    let chunks = Layout::vertical([
        Constraint::Length(2),                      // Title
        Constraint::Length(modes.len() as u16 + 1), // Permission list + header
        Constraint::Length(2),                      // Model (header + row)
        Constraint::Length(2),                      // Effort (header + row)
        Constraint::Length(1),                      // Spacer
        Constraint::Length(1),                      // Footer buttons
    ])
    // Margin = border (1) + one cell of padding on every side.
    .split(popup_area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 2,
    }));

    render_title(frame, chunks[0]);
    render_permission_section(frame, chunks[1], state, modes);
    render_model_section(frame, chunks[2], state, models);
    render_effort_section(frame, chunks[3], state, efforts);
    render_footer(frame, chunks[5], state);
}

fn render_title(frame: &mut Frame, area: Rect) {
    let text = Paragraph::new("Klick oder Tab/Pfeiltasten — die Auswahl wird pro Projekt gemerkt.")
        .style(Style::default().fg(Color::White));
    frame.render_widget(text, area);
}

fn render_permission_section(
    frame: &mut Frame,
    area: Rect,
    state: &mut PermissionModeState,
    modes: &[ClaudePermissionMode],
) {
    let is_active = state.section == DialogSection::Permission;
    let header_style = section_header_style(is_active);
    let header = Line::from(vec![
        Span::styled("[ Permission Mode ]", header_style),
        Span::styled(
            if is_active {
                "  ↑↓ wählen · Doppelklick startet"
            } else {
                ""
            },
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let mut lines: Vec<ListItem> = Vec::new();
    lines.push(ListItem::new(header));

    for (i, mode) in modes.iter().enumerate() {
        let is_selected = i == state.permission_selected;
        let is_yolo = mode.is_yolo();
        let selector = if is_selected && is_active {
            "▸ "
        } else if is_selected {
            "● "
        } else {
            "  "
        };

        let name_color = if is_yolo {
            Color::Red
        } else if is_selected {
            Color::Yellow
        } else {
            Color::White
        };

        let line = Line::from(vec![
            Span::styled(
                selector,
                Style::default().fg(if is_selected {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                format!("{:<18}", mode.name()),
                Style::default()
                    .fg(name_color)
                    .add_modifier(if is_selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(" - ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                mode.description_de(),
                Style::default().fg(if is_yolo { Color::Red } else { Color::Gray }),
            ),
        ]);
        lines.push(ListItem::new(line));
    }

    // Row i of the list sits one line below the header; rows cut off by a
    // too-small terminal get no click area.
    let bottom = area.y + area.height;
    state.hit_areas.permission_rows = (0..modes.len() as u16)
        .map(|i| area.y + 1 + i)
        .filter(|y| *y < bottom)
        .map(|y| Rect::new(area.x, y, area.width, 1))
        .collect();

    frame.render_widget(List::new(lines), area);
}

fn render_model_section(
    frame: &mut Frame,
    area: Rect,
    state: &mut PermissionModeState,
    models: &[ClaudeModel],
) {
    let is_active = state.section == DialogSection::Model;
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("[ Model ]", section_header_style(is_active)),
        Span::styled(
            if is_active { "  ←→ wählen" } else { "" },
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    frame.render_widget(header, chunks[0]);

    const PREFIX: &str = "  ";
    let spans = radio_spans(
        PREFIX,
        models.iter().map(|m| m.name()),
        state.model_selected,
        is_active,
    );
    state.hit_areas.model_items = item_rects(
        chunks[1],
        models.iter().map(|m| radio_width(PREFIX, m.name())),
    );
    frame.render_widget(Paragraph::new(Line::from(spans)), chunks[1]);
}

fn render_effort_section(
    frame: &mut Frame,
    area: Rect,
    state: &mut PermissionModeState,
    efforts: &[ClaudeEffort],
) {
    let is_active = state.section == DialogSection::Effort;
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("[ Effort ]", section_header_style(is_active)),
        Span::styled(
            if is_active { "  ←→ wählen" } else { "" },
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    frame.render_widget(header, chunks[0]);

    const PREFIX: &str = " ";
    let spans = radio_spans(
        PREFIX,
        efforts.iter().map(|e| e.name()),
        state.effort_selected,
        is_active,
    );
    state.hit_areas.effort_items = item_rects(
        chunks[1],
        efforts.iter().map(|e| radio_width(PREFIX, e.name())),
    );
    frame.render_widget(Paragraph::new(Line::from(spans)), chunks[1]);
}

fn radio_spans<'a>(
    prefix: &str,
    names: impl Iterator<Item = &'a str>,
    selected: usize,
    is_active: bool,
) -> Vec<Span<'static>> {
    names
        .enumerate()
        .map(|(i, name)| {
            let is_selected = i == selected;
            let marker = if is_selected { "(•)" } else { "( )" };
            let style = if is_selected {
                Style::default()
                    .fg(if is_active {
                        Color::Yellow
                    } else {
                        Color::White
                    })
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            Span::styled(format!("{}{} {}", prefix, marker, name), style)
        })
        .collect()
}

fn render_footer(frame: &mut Frame, area: Rect, state: &mut PermissionModeState) {
    // (key, label, style of key); the first two are clickable buttons.
    const CONFIRM: (&str, &str) = (" Enter ", " Starten ");
    const CANCEL: (&str, &str) = (" Esc ", " Gemerkte Werte ");
    const TAB: (&str, &str) = (" Tab ", " Sektion");
    const GAP: &str = "  ";

    let width = |(key, label): (&str, &str)| (key.chars().count() + label.chars().count()) as u16;
    let rects = item_rects(
        area,
        [width(CONFIRM), GAP.len() as u16, width(CANCEL)].into_iter(),
    );
    state.hit_areas.confirm = rects[0];
    state.hit_areas.cancel = rects[2];

    let footer = Line::from(vec![
        Span::styled(CONFIRM.0, Style::default().bg(Color::Cyan).fg(Color::Black)),
        Span::styled(
            CONFIRM.1,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(GAP),
        Span::styled(
            CANCEL.0,
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(CANCEL.1),
        Span::raw(GAP),
        Span::styled(TAB.0, Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::styled(TAB.1, Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(footer), area);
}

fn section_header_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn rendered_state() -> PermissionModeState {
        let mut state = PermissionModeState::default();
        state.open_with_defaults(StartupOptions::default(), "demo".to_string());
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &mut state))
            .unwrap();
        state
    }

    fn center(rect: Rect) -> (u16, u16) {
        (rect.x + rect.width / 2, rect.y)
    }

    #[test]
    fn every_option_and_button_is_clickable() {
        let state = rendered_state();
        let areas = state.hit_areas.clone();
        assert_eq!(
            areas.permission_rows.len(),
            ClaudePermissionMode::all().len()
        );
        assert_eq!(areas.model_items.len(), ClaudeModel::all().len());
        assert_eq!(areas.effort_items.len(), ClaudeEffort::all().len());

        for (i, rect) in areas.permission_rows.iter().enumerate() {
            let (x, y) = center(*rect);
            assert_eq!(state.hit_test(x, y), DialogClick::Permission(i));
        }
        for (i, rect) in areas.model_items.iter().enumerate() {
            let (x, y) = center(*rect);
            assert_eq!(state.hit_test(x, y), DialogClick::Model(i));
        }
        for (i, rect) in areas.effort_items.iter().enumerate() {
            let (x, y) = center(*rect);
            assert_eq!(state.hit_test(x, y), DialogClick::Effort(i));
        }
        let (x, y) = center(areas.confirm);
        assert_eq!(state.hit_test(x, y), DialogClick::Confirm);
        let (x, y) = center(areas.cancel);
        assert_eq!(state.hit_test(x, y), DialogClick::Cancel);
        assert_eq!(state.hit_test(0, 0), DialogClick::None);
    }

    #[test]
    fn model_click_areas_cover_the_rendered_labels() {
        // The "Opus" label must sit inside the area that selects Opus.
        let mut state = PermissionModeState::default();
        state.open_with_defaults(StartupOptions::default(), String::new());
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &mut state))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let opus = ClaudeModel::all()
            .iter()
            .position(|m| m.name() == "Opus")
            .expect("Opus is offered");
        let rect = state.hit_areas.model_items[opus];
        let text: String = (rect.x..rect.x + rect.width)
            .map(|x| buffer[(x, rect.y)].symbol().to_string())
            .collect();
        assert!(text.contains("Opus"), "area shows {text:?}");
    }

    #[test]
    fn clicks_select_options_and_switch_section() {
        let mut state = rendered_state();
        state.select_model(2);
        assert_eq!(state.section, DialogSection::Model);
        assert_eq!(state.model_selected, 2);
        state.select_effort(3);
        assert_eq!(state.section, DialogSection::Effort);
        assert_eq!(state.effort_selected, 3);
        // Out-of-range indices are ignored.
        state.select_effort(99);
        assert_eq!(state.effort_selected, 3);
    }

    #[test]
    fn second_click_on_same_permission_row_is_a_double_click() {
        let mut state = rendered_state();
        let t0 = Instant::now();
        assert!(!state.click_permission_at(2, t0));
        assert_eq!(state.permission_selected, 2);
        assert!(state.click_permission_at(2, t0 + Duration::from_millis(200)));
    }

    #[test]
    fn slow_or_different_second_click_is_no_double_click() {
        let mut state = rendered_state();
        let t0 = Instant::now();
        assert!(!state.click_permission_at(1, t0));
        assert!(!state.click_permission_at(2, t0 + Duration::from_millis(100)));
        assert!(!state.click_permission_at(2, t0 + Duration::from_millis(900)));
    }

    #[test]
    fn open_preselects_the_remembered_values() {
        let mut state = PermissionModeState::default();
        state.open_with_defaults(
            StartupOptions {
                permission_mode: ClaudePermissionMode::Auto,
                model: ClaudeModel::Opus,
                effort: ClaudeEffort::High,
            },
            "p".to_string(),
        );
        assert_eq!(state.selected_permission_mode(), ClaudePermissionMode::Auto);
        assert_eq!(state.selected_model(), ClaudeModel::Opus);
        assert_eq!(state.selected_effort(), ClaudeEffort::High);
    }
}
