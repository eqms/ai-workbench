//! File-browser pane key handling — j/k navigation, PageUp/PageDown and
//! Home/End (`g`/`G`) jumps, h/l directory entry, `H`/`L` re-rooting the tree,
//! `o`/`O` external open, `.` toggle hidden, `i` add-to-gitignore,
//! Ctrl+A toggle autosave, `q` quit.

use crossterm::event::KeyCode;

use super::super::App;

impl App {
    pub(super) fn handle_file_browser_pane_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.file_browser.down();
                self.update_preview();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.file_browser.up();
                self.update_preview();
            }
            KeyCode::PageUp => {
                self.file_browser
                    .page_up(self.files_pane_height.max(1) as usize);
                self.update_preview();
            }
            KeyCode::PageDown => {
                self.file_browser
                    .page_down(self.files_pane_height.max(1) as usize);
                self.update_preview();
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.file_browser.select_first();
                self.update_preview();
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.file_browser.select_last();
                self.update_preview();
            }
            // Make the selected directory the tree root / move the root up.
            KeyCode::Char('L') => {
                if self.file_browser.root_into_selected() {
                    self.update_preview();
                    self.sync_terminals();
                    self.check_repo_change();
                }
            }
            KeyCode::Char('H') => {
                self.file_browser.root_up();
                self.update_preview();
                self.sync_terminals();
                self.check_repo_change();
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(_path) = self.file_browser.enter_selected() {
                    // A file is already shown in the preview; hand it the focus
                    // so Enter leads somewhere instead of doing nothing.
                    if self.show_preview {
                        self.active_pane = crate::types::PaneId::Preview;
                    }
                } else {
                    self.update_preview();
                    self.sync_terminals();
                    self.check_repo_change();
                }
            }
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                self.file_browser.go_parent();
                self.update_preview();
                self.sync_terminals();
                self.check_repo_change();
            }
            KeyCode::Char('o') => {
                if let Some(path) = self.file_browser.selected_file() {
                    self.open_in_browser(&path);
                }
            }
            KeyCode::Char('O') => {
                let dir = self.file_browser.selection_dir();
                let outcome = crate::browser::remote_open::open_or_transfer(
                    &dir,
                    crate::browser::remote_open::OpenKind::Directory,
                    &self.config.ui.browser,
                    &self.config.ui.remote_transfer,
                );
                self.set_open_flash(outcome);
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('.') => {
                self.file_browser.show_hidden = !self.file_browser.show_hidden;
                self.file_browser.refresh();
                self.update_preview();
            }
            KeyCode::Char('i') => {
                if let Some(path) = self.file_browser.selected_file() {
                    self.add_to_gitignore(&path);
                }
            }
            // Ctrl+A = Toggle Autosave
            KeyCode::Char('\x01') => {
                self.config.ui.autosave = !self.config.ui.autosave;
                let _ = crate::config::save_config(&self.config);
            }
            _ => {}
        }
    }
}
