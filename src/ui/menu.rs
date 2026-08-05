use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAction {
    None,
    NewFile,
    NewDirectory,
    RenameFile,
    DuplicateFile,
    CopyFileTo,
    MoveFileTo,
    DeleteFile,
    CopyAbsolutePath,
    CopyRelativePath,
    GoToPath,
    AddToGitignore,
    ExportFile,
    GitPull,
}

/// Number of selectable entries. Kept next to the `action()` mapping and the
/// `items` list in `render` — all three must agree, and a mismatch here silently
/// makes the last entry unreachable.
const ITEM_COUNT: usize = 13;

#[derive(Default)]
pub struct MenuBar {
    pub visible: bool,
    pub selected: usize,
}

impl MenuBar {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.selected = 0;
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % ITEM_COUNT;
    }

    pub fn prev(&mut self) {
        if self.selected == 0 {
            self.selected = ITEM_COUNT - 1;
        } else {
            self.selected -= 1;
        }
    }

    pub fn action(&self) -> MenuAction {
        match self.selected {
            0 => MenuAction::NewFile,
            1 => MenuAction::NewDirectory,
            2 => MenuAction::RenameFile,
            3 => MenuAction::DuplicateFile,
            4 => MenuAction::CopyFileTo,
            5 => MenuAction::MoveFileTo,
            6 => MenuAction::DeleteFile,
            7 => MenuAction::CopyAbsolutePath,
            8 => MenuAction::CopyRelativePath,
            9 => MenuAction::GoToPath,
            10 => MenuAction::AddToGitignore,
            11 => MenuAction::ExportFile,
            12 => MenuAction::GitPull,
            _ => MenuAction::None,
        }
    }
}

pub fn render(f: &mut Frame, area: Rect, menu: &MenuBar) {
    if !menu.visible {
        return;
    }

    let items = vec![
        ("n", "New File"),
        ("N", "New Directory"),
        ("r", "Rename"),
        ("u", "Duplicate"),
        ("c", "Copy to..."),
        ("m", "Move to..."),
        ("d", "Delete"),
        ("y", "Copy Abs Path"),
        ("Y", "Copy Rel Path"),
        ("g", "Go to Path"),
        ("i", "Add to .gitignore"),
        ("x", "Export Markdown/PDF"),
        ("p", "Git Pull"),
    ];
    debug_assert_eq!(items.len(), ITEM_COUNT, "menu items and ITEM_COUNT drifted");

    // Menu popup in center-top
    let width = 40u16;
    let height = (items.len() + 2) as u16;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + 2;
    let popup_area = Rect::new(x, y, width, height);

    // Clear background
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" File Menu (Esc to close) ")
        .style(Style::default().bg(Color::DarkGray));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    for (i, (key, label)) in items.iter().enumerate() {
        let style = if i == menu.selected {
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let line = Line::from(vec![
            Span::styled(format!(" [{}] ", key), Style::default().fg(Color::Yellow)),
            Span::styled(*label, style),
        ]);

        let item_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        f.render_widget(
            Paragraph::new(line).style(Style::default().bg(Color::DarkGray)),
            item_area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `next`/`prev` wrap over `ITEM_COUNT`, so every index must map to a real
    /// action — an off-by-one here makes the last entry silently unreachable.
    #[test]
    fn every_index_maps_to_an_action() {
        for i in 0..ITEM_COUNT {
            let menu = MenuBar {
                visible: true,
                selected: i,
            };
            assert_ne!(menu.action(), MenuAction::None, "index {i} has no action");
        }
    }

    #[test]
    fn wrapping_covers_the_full_list() {
        let mut menu = MenuBar {
            visible: true,
            selected: 0,
        };
        menu.prev();
        assert_eq!(menu.selected, ITEM_COUNT - 1);
        assert_eq!(menu.action(), MenuAction::GitPull);
        menu.next();
        assert_eq!(menu.selected, 0);
        assert_eq!(menu.action(), MenuAction::NewFile);
    }
}
