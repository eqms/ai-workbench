//! Immediate boot screen shown between `ratatui::init()` and the first real
//! frame of the event loop.
//!
//! `ratatui::init()` enters the alternate screen (blanking the terminal), but
//! the kitty-keyboard probe and `App::new()` still run before `App::run`'s
//! first draw — previously a multi-second black screen. `main.rs` calls
//! [`render`] via `terminal.draw(..)` around those phases so the user sees the
//! wordmark and a status line instantly.
//!
//! Visually this matches the *stabilized* end state of the intro animation
//! (`ui::intro`, cyan/bold banner + green version line), so boot screen and
//! intro read as one continuous reveal.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use super::intro::{build_banner, center_pad};

/// Render the boot screen: centered block banner, version, and status line.
pub fn render(frame: &mut Frame, area: Rect, status: &str, version: &str) {
    frame.render_widget(Clear, area);

    let banner = build_banner();
    let banner_width = banner.first().map(|r| r.chars().count()).unwrap_or(0);
    let version_text = format!("v{}", version);

    // banner rows + spacer + version + spacer + status
    let block_h = banner.len() as u16 + 4;
    let block_w = banner_width as u16 + 4;

    // Small-terminal fallback: skip the block art, show a simple wordmark.
    if area.width < block_w || area.height < block_h {
        render_fallback(frame, area, status, &version_text);
        return;
    }

    let mut lines: Vec<Line> = Vec::with_capacity(block_h as usize);
    for row in &banner {
        lines.push(Line::from(Span::styled(
            row.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        center_pad(&version_text, banner_width),
        Style::default().fg(Color::Green),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        center_pad(status, banner_width),
        Style::default().fg(Color::DarkGray),
    )));

    let block_x = (area.width.saturating_sub(block_w)) / 2;
    let block_y = (area.height.saturating_sub(block_h)) / 2;
    let block_rect = Rect::new(block_x, block_y, block_w, block_h);
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), block_rect);
}

/// Minimal fallback for terminals too small for the block banner.
fn render_fallback(frame: &mut Frame, area: Rect, status: &str, version: &str) {
    let lines = vec![
        Line::from(Span::styled(
            "A I   W O R K B E N C H",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            version.to_string(),
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            status.to_string(),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let h = 3u16;
    let y = (area.height.saturating_sub(h)) / 2;
    let rect = Rect::new(area.x, y, area.width, h);
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_never_panics_across_sizes() {
        use ratatui::{backend::TestBackend, Terminal};
        // Large (block banner), medium (also banner), tiny (fallback path),
        // and a degenerate 1x1 buffer must all render without panic/overflow.
        for (w, h) in [(100u16, 30u16), (72, 12), (40, 10), (1, 1)] {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|f| render(f, f.area(), "probing terminal...", "1.9.0"))
                .expect("render must not panic");
        }
    }

    #[test]
    fn banner_and_fallback_show_status() {
        use ratatui::{backend::TestBackend, Terminal};
        for (w, h) in [(100u16, 30u16), (40, 10)] {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|f| render(f, f.area(), "initializing panes...", "1.9.0"))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
            assert!(
                content.contains("initializing panes..."),
                "status line must be visible at {}x{}",
                w,
                h
            );
            assert!(content.contains("v1.9.0"), "version must be visible");
        }
    }
}
