//! Startup intro animation: "Cyberpunk Glitch & Scanline Reveal".
//!
//! A short (~4.5 s) full-screen overlay shown once at startup. An "AI WORKBENCH"
//! block logo flickers with glitch corruption, a bright cyan scanline sweeps top
//! to bottom "repairing" the logo, then it stabilizes in the branding colors.
//!
//! The animation is purely time-driven: the render fn derives the current frame
//! from `IntroState::start_time.elapsed()`, so no tick/frame-counter state is
//! needed — the 16 ms event-loop redraw (`App::run`) supplies the frames. Glitch
//! randomness comes from a tiny inline xorshift PRNG (no `rand` dependency),
//! re-seeded each frame from `(elapsed_millis, row)` so it flickers on every
//! redraw yet stays deterministic for a given instant.

use std::time::Instant;

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

/// Wordmark text rendered as a block-letter banner.
const LOGO_TEXT: &str = "AI WORKBENCH";

/// Duration of the pure-glitch phase (seconds).
pub const GLITCH_DURATION: f32 = 0.9;
/// Duration of the scanline-sweep phase (seconds).
pub const SWEEP_DURATION: f32 = 2.4;
/// Duration of the stabilization phase (seconds).
pub const STABILIZE_DURATION: f32 = 1.2;
/// Total animation length after which the intro auto-dismisses (seconds).
pub const TOTAL_DURATION: f32 = GLITCH_DURATION + SWEEP_DURATION + STABILIZE_DURATION;

/// Glitch substitution characters (matte cyberpunk corruption).
const GLITCH_POOL: [char; 9] = ['@', '#', '$', '%', '&', '░', '▒', '▓', '█'];

/// The three temporal phases of the animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Logo flickers with corruption, no scanline yet.
    Glitch,
    /// Scanline sweeps top→bottom; above = repaired, below = glitch.
    Sweep,
    /// Logo fully repaired and stable.
    Stabilize,
}

/// Runtime state for the startup intro animation.
#[derive(Debug, Clone)]
pub struct IntroState {
    /// Whether the overlay is currently shown.
    pub visible: bool,
    /// When the animation started (anchors all timing).
    start_time: Instant,
}

impl IntroState {
    /// Create a new intro. `enabled == false` yields an already-hidden intro
    /// (used when `config.ui.intro_animation` is off), so callers can always
    /// construct one unconditionally.
    pub fn new(enabled: bool) -> Self {
        Self {
            visible: enabled,
            start_time: Instant::now(),
        }
    }

    /// Skip the animation immediately (any key / click).
    pub fn dismiss(&mut self) {
        self.visible = false;
    }

    /// Re-anchor the animation clock to now. Called at the top of `App::run`
    /// so the full duration plays from the first visible frame — without
    /// this, time spent in `App::new` (constructed before the first draw)
    /// silently eats into the animation budget.
    pub fn restart(&mut self) {
        self.start_time = Instant::now();
    }

    /// Auto-dismiss once the total duration has elapsed. Called each loop
    /// iteration; cheap and non-blocking.
    pub fn tick(&mut self) {
        if self.visible && self.start_time.elapsed().as_secs_f32() >= TOTAL_DURATION {
            self.visible = false;
        }
    }

    /// Seconds since the animation started.
    fn elapsed(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32()
    }
}

/// Pure phase selection from elapsed seconds (unit-testable).
pub fn phase_for(elapsed: f32) -> Phase {
    if elapsed < GLITCH_DURATION {
        Phase::Glitch
    } else if elapsed < GLITCH_DURATION + SWEEP_DURATION {
        Phase::Sweep
    } else {
        Phase::Stabilize
    }
}

/// Whether the animation has run past its total duration (unit-testable).
pub fn is_expired(elapsed: f32) -> bool {
    elapsed >= TOTAL_DURATION
}

/// One xorshift64 step. Deterministic PRNG, no external crate.
fn next_rng(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// 5-row block glyph for a single uppercase letter. Unknown chars render blank.
fn glyph(c: char) -> [&'static str; 5] {
    match c {
        'A' => [" ███ ", "█   █", "█████", "█   █", "█   █"],
        'I' => ["███", " █ ", " █ ", " █ ", "███"],
        'W' => ["█   █", "█   █", "█ █ █", "█ █ █", " █ █ "],
        'O' => [" ███ ", "█   █", "█   █", "█   █", " ███ "],
        'R' => ["████ ", "█   █", "████ ", "█  █ ", "█   █"],
        'K' => ["█   █", "█  █ ", "███  ", "█  █ ", "█   █"],
        'B' => ["████ ", "█   █", "████ ", "█   █", "████ "],
        'E' => ["█████", "█    ", "████ ", "█    ", "█████"],
        'N' => ["█   █", "██  █", "█ █ █", "█  ██", "█   █"],
        'C' => [" ████", "█    ", "█    ", "█    ", " ████"],
        'H' => ["█   █", "█   █", "█████", "█   █", "█   █"],
        _ => ["     ", "     ", "     ", "     ", "     "],
    }
}

/// Assemble the block banner for [`LOGO_TEXT`] as 5 equal-width rows.
/// `pub(crate)` so the boot screen renders the identical wordmark.
pub(crate) fn build_banner() -> Vec<String> {
    let mut rows = vec![String::new(); 5];
    for (i, c) in LOGO_TEXT.chars().enumerate() {
        if c == ' ' {
            // Word gap.
            for r in rows.iter_mut() {
                r.push_str("   ");
            }
            continue;
        }
        // 1-column separator between adjacent glyphs.
        if i > 0 {
            for r in rows.iter_mut() {
                r.push(' ');
            }
        }
        let g = glyph(c);
        for (r, gr) in rows.iter_mut().zip(g.iter()) {
            r.push_str(gr);
        }
    }
    rows
}

/// Apply glitch corruption to one banner row for the given frame seed.
///
/// Returns the (possibly shifted + corrupted) string. `rng` is threaded so the
/// whole frame shares one deterministic stream.
fn glitch_row(original: &str, rng: &mut u64) -> String {
    // ~20% chance to shift the whole line right by 1–3 cells.
    let mut out = String::new();
    if next_rng(rng) % 100 < 20 {
        let offset = 1 + (next_rng(rng) % 3) as usize;
        for _ in 0..offset {
            out.push(' ');
        }
    }
    for ch in original.chars() {
        if !ch.is_whitespace() && next_rng(rng) % 100 < 15 {
            let idx = (next_rng(rng) % GLITCH_POOL.len() as u64) as usize;
            out.push(GLITCH_POOL[idx]);
        } else {
            out.push(ch);
        }
    }
    out
}

/// Center a short string inside `width` columns with leading spaces.
/// `pub(crate)` so the boot screen shares the same centering.
pub(crate) fn center_pad(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.to_string();
    }
    let pad = (width - len) / 2;
    format!("{}{}", " ".repeat(pad), text)
}

/// Render the intro overlay. Full-screen: clears `area` to a black canvas and
/// draws the centered animated banner. `state` is read-only — timing derives
/// from its `start_time`.
pub fn render(frame: &mut Frame, area: Rect, state: &IntroState) {
    // Black canvas over everything (panes, wizard, footer).
    frame.render_widget(Clear, area);

    let banner = build_banner();
    let banner_height = banner.len();
    let banner_width = banner.first().map(|r| r.chars().count()).unwrap_or(0);
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));

    let elapsed = state.elapsed();
    let phase = phase_for(elapsed);

    // Small-terminal fallback: skip the block art, show a simple styled wordmark.
    let block_w = banner_width as u16 + 4;
    let block_h = banner_height as u16 + 2;
    if area.width < block_w || area.height < block_h {
        render_fallback(frame, area, &version, phase);
        return;
    }

    // Scanline position within the banner (only meaningful during Sweep).
    let sweep_progress = if elapsed < GLITCH_DURATION {
        0.0
    } else {
        ((elapsed - GLITCH_DURATION) / SWEEP_DURATION).clamp(0.0, 1.0)
    };
    let scanline_y = (sweep_progress * banner_height as f32) as usize;
    let stabilized = matches!(phase, Phase::Stabilize);

    // Per-frame RNG seed: flickers each redraw, deterministic for an instant.
    let base_seed = ((elapsed * 1000.0) as u64).max(1);

    let mut lines: Vec<Line> = Vec::with_capacity(banner_height + 2);
    for (y, original) in banner.iter().enumerate() {
        let rendered: Line = if stabilized || (sweep_progress > 0.0 && y < scanline_y) {
            // Repaired: solid branding cyan.
            Line::from(Span::styled(
                original.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
        } else if sweep_progress > 0.0 && y == scanline_y {
            // The glowing scan beam: a bright full-width bar.
            let bar: String = "█".repeat(banner_width);
            Line::from(Span::styled(
                bar,
                Style::default().fg(Color::White).bg(Color::Cyan),
            ))
        } else {
            // Glitch: matte, corrupted, occasionally offset.
            let mut seed = base_seed ^ ((y as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let text = glitch_row(original, &mut seed);
            Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))
        };
        lines.push(rendered);
    }

    // Blank spacer + version line.
    lines.push(Line::from(""));
    let version_style = if stabilized {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    lines.push(Line::from(Span::styled(
        center_pad(&version, banner_width),
        version_style,
    )));

    // Left-aligned inside a centered fixed-width rect so glitch offsets survive.
    let block_x = (area.width.saturating_sub(block_w)) / 2;
    let block_y = (area.height.saturating_sub(block_h)) / 2;
    let block_rect = Rect::new(block_x, block_y, block_w, block_h);
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), block_rect);
}

/// Minimal fallback for terminals too small for the block banner.
fn render_fallback(frame: &mut Frame, area: Rect, version: &str, phase: Phase) {
    let color = if matches!(phase, Phase::Stabilize) {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let lines = vec![
        Line::from(Span::styled(
            "A I   W O R K B E N C H",
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            version.to_string(),
            Style::default().fg(Color::Green),
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
    fn new_disabled_is_hidden() {
        assert!(!IntroState::new(false).visible);
        assert!(IntroState::new(true).visible);
    }

    #[test]
    fn phase_boundaries() {
        assert_eq!(phase_for(0.1), Phase::Glitch);
        assert_eq!(phase_for(0.89), Phase::Glitch);
        assert_eq!(phase_for(0.9), Phase::Sweep);
        assert_eq!(phase_for(2.0), Phase::Sweep);
        assert_eq!(phase_for(3.29), Phase::Sweep);
        assert_eq!(phase_for(3.31), Phase::Stabilize);
        assert_eq!(phase_for(4.4), Phase::Stabilize);
        assert_eq!(phase_for(4.6), Phase::Stabilize);
    }

    #[test]
    fn expiry_boundary() {
        assert!(!is_expired(4.49));
        assert!(is_expired(TOTAL_DURATION));
        assert!(is_expired(4.6));
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = 12345;
        let mut b = 12345;
        for _ in 0..10 {
            assert_eq!(next_rng(&mut a), next_rng(&mut b));
        }
        // And it actually changes state.
        let mut s = 1;
        assert_ne!(next_rng(&mut s), next_rng(&mut s));
    }

    #[test]
    fn banner_rows_equal_width() {
        let banner = build_banner();
        assert_eq!(banner.len(), 5);
        let w = banner[0].chars().count();
        assert!(w > 0);
        for row in &banner {
            assert_eq!(
                row.chars().count(),
                w,
                "all banner rows must be equal width"
            );
        }
    }

    #[test]
    fn glitch_row_preserves_length_without_offset() {
        // Seed chosen so the 20%-offset branch does not fire on the first draw.
        let original = "█   █";
        let mut seed = 7;
        let out = glitch_row(original, &mut seed);
        // Length is >= original (offset may add leading spaces); never shorter.
        assert!(out.chars().count() >= original.chars().count());
    }

    #[test]
    fn center_pad_centers() {
        assert_eq!(center_pad("ab", 6), "  ab");
        assert_eq!(center_pad("abcdef", 4), "abcdef"); // no truncation
    }

    #[test]
    fn render_never_panics_across_sizes() {
        use ratatui::{backend::TestBackend, Terminal};
        // Large (block banner), medium (also banner), tiny (fallback path), and
        // a degenerate 1x1 buffer must all render without panic/overflow.
        for (w, h) in [(100u16, 30u16), (72, 12), (40, 10), (1, 1)] {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            let state = IntroState::new(true);
            terminal
                .draw(|f| render(f, f.area(), &state))
                .expect("render must not panic");
        }
    }
}
