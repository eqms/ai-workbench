//! Wizard UI rendering

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::setup::wizard::{WizardField, WizardState, WizardStep};

/// Render the installation wizard
pub fn render(frame: &mut Frame, area: Rect, state: &WizardState) {
    // Calculate centered popup area (70% width, 80% height)
    let popup_width = (area.width as f32 * 0.7) as u16;
    let popup_height = (area.height as f32 * 0.8) as u16;

    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear background
    frame.render_widget(Clear, popup_area);

    // Build title with step indicator. Step counts are dynamic — the SSH
    // image-paste step only appears when running over SSH.
    let title = format!(
        " {} - Step {}/{} ",
        state.step.title(),
        state.current_step_number(),
        state.total_steps()
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Render step-specific content
    match state.step {
        WizardStep::Welcome => render_welcome(frame, inner),
        WizardStep::Dependencies => render_dependencies(frame, inner, state),
        WizardStep::ShellSelection => render_shell_selection(frame, inner, state),
        WizardStep::ClaudeConfig => render_claude_config(frame, inner, state),
        WizardStep::SshImagePaste => render_ssh_image_paste(frame, inner, state),
        WizardStep::Confirmation => render_confirmation(frame, inner, state),
        WizardStep::Complete => render_complete(frame, inner),
    }

    // Render navigation footer
    render_footer(frame, popup_area, state);
}

fn render_welcome(frame: &mut Frame, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);

    // Title banner
    let banner = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Welcome to ", Style::default()),
            Span::styled(
                "AI Workbench",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ])
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(banner, chunks[0]);

    // Description
    let desc = Paragraph::new(vec![
        Line::from(""),
        Line::from("This wizard will help you configure your development environment."),
        Line::from(""),
        Line::from("We'll check for the following tools:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::Cyan)),
            Span::raw("Git (required)"),
        ]),
        Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::Cyan)),
            Span::raw("AI backend CLI: claude / opencode / pi"),
        ]),
        Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::Cyan)),
            Span::raw("LazyGit (optional)"),
        ]),
        Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::Cyan)),
            Span::raw("Available shells (bash, zsh, fish)"),
        ]),
    ])
    .wrap(Wrap { trim: false });
    frame.render_widget(desc, chunks[2]);

    // Hint
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Press ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(" to continue or ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" to skip wizard", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(hint, chunks[3]);
}

fn render_dependencies(frame: &mut Frame, area: Rect, state: &WizardState) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);

    // Header
    let header =
        Paragraph::new("Checking installed tools...").style(Style::default().fg(Color::Yellow));
    frame.render_widget(header, chunks[0]);

    // Dependencies list
    let mut items: Vec<ListItem> = Vec::new();

    // Git
    let git_status = if state.deps.git.found {
        let version = state.deps.git.version.as_deref().unwrap_or("unknown");
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled("git", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" - {}", version)),
        ])
    } else {
        Line::from(vec![
            Span::styled("✗ ", Style::default().fg(Color::Red)),
            Span::styled("git", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(" - NOT FOUND (required)", Style::default().fg(Color::Red)),
        ])
    };
    items.push(ListItem::new(git_status));

    // Claude CLI
    let claude_status = if state.deps.claude_cli.found {
        let version = state
            .deps
            .claude_cli
            .version
            .as_deref()
            .unwrap_or("unknown");
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled("claude", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" - {}", version)),
        ])
    } else {
        Line::from(vec![
            Span::styled("○ ", Style::default().fg(Color::Yellow)),
            Span::styled("claude", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                " - not found (optional)",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };
    items.push(ListItem::new(claude_status));

    // OpenCode / Pi backends — optional, same "found/version" rendering.
    let optional_line = |name: &str, dep: &crate::setup::dependency_checker::DependencyStatus| {
        if dep.found {
            let version = dep.version.as_deref().unwrap_or("unknown");
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::styled(
                    name.to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" - {}", version)),
            ])
        } else {
            Line::from(vec![
                Span::styled("○ ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    name.to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " - not found (optional)",
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        }
    };
    items.push(ListItem::new(optional_line(
        "opencode",
        &state.deps.opencode_cli,
    )));
    items.push(ListItem::new(optional_line("pi", &state.deps.pi_cli)));
    items.push(ListItem::new(optional_line("codex", &state.deps.codex_cli)));

    // LazyGit
    let lazygit_status = if state.deps.lazygit.found {
        let version = state.deps.lazygit.version.as_deref().unwrap_or("unknown");
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled("lazygit", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" - {}", version)),
        ])
    } else {
        Line::from(vec![
            Span::styled("○ ", Style::default().fg(Color::Yellow)),
            Span::styled("lazygit", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                " - not found (optional)",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };
    items.push(ListItem::new(lazygit_status));

    // Shells header
    items.push(ListItem::new(Line::from("")));
    items.push(ListItem::new(Line::from(vec![Span::styled(
        "Available Shells:",
        Style::default().add_modifier(Modifier::BOLD),
    )])));

    for shell in &state.deps.shells {
        let path = shell
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        items.push(ListItem::new(Line::from(vec![
            Span::styled("  ✓ ", Style::default().fg(Color::Green)),
            Span::raw(&shell.name),
            Span::styled(format!(" ({})", path), Style::default().fg(Color::DarkGray)),
        ])));
    }

    let list = List::new(items);
    frame.render_widget(list, chunks[1]);

    // Summary
    let (found, missing_req, missing_opt) = state.deps.summary();
    let summary_style = if missing_req > 0 {
        Style::default().fg(Color::Red)
    } else if missing_opt > 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };

    let summary = Paragraph::new(format!(
        "Found: {} | Missing required: {} | Missing optional: {}",
        found, missing_req, missing_opt
    ))
    .style(summary_style)
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(summary, chunks[2]);
}

fn render_shell_selection(frame: &mut Frame, area: Rect, state: &WizardState) {
    let chunks = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).split(area);

    let header = Paragraph::new("Select your preferred shell for the terminal pane:");
    frame.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = state
        .available_shells
        .iter()
        .enumerate()
        .map(|(i, shell)| {
            let prefix = if i == state.selected_shell_idx {
                "● "
            } else {
                "○ "
            };
            let style = if i == state.selected_shell_idx {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(shell.as_str(), style),
            ]))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, chunks[1]);
}

fn render_claude_config(frame: &mut Frame, area: Rect, state: &WizardState) {
    use crate::backend::AiBackend;

    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(2), // backend selector
        Constraint::Length(3), // Claude path
        Constraint::Length(3), // OpenCode path
        Constraint::Length(3), // Pi path
        Constraint::Length(3), // Codex path
        Constraint::Length(3), // LazyGit path
        Constraint::Min(1),    // hint
    ])
    .split(area);

    let header = Paragraph::new("Configure default backend & tool paths:")
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(header, chunks[0]);

    // Default-backend selector line
    let mut selector_spans = vec![Span::styled(
        "Default backend: ",
        Style::default().add_modifier(Modifier::BOLD),
    )];
    for (i, b) in AiBackend::all().iter().enumerate() {
        let is_selected = *b == state.selected_backend;
        let style = if is_selected {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        selector_spans.push(Span::styled(
            format!("[{}] {}", i + 1, b.short_label()),
            style,
        ));
        selector_spans.push(Span::raw("   "));
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(selector_spans),
            Line::from(Span::styled(
                "  press 1/2/3/4 to choose",
                Style::default().fg(Color::DarkGray),
            )),
        ]),
        chunks[1],
    );

    // CLI path fields (focused_field: 0=Claude, 1=OpenCode, 2=Pi, 3=Codex, 4=LazyGit)
    render_path_field(
        frame,
        chunks[2],
        "Claude CLI",
        &state.claude_path,
        state.deps.claude_cli.found,
        state.focused_field == 0,
        state.editing_field == Some(WizardField::ClaudePath),
        &state.input_buffer,
    );
    render_path_field(
        frame,
        chunks[3],
        "OpenCode CLI",
        &state.opencode_path,
        state.deps.opencode_cli.found,
        state.focused_field == 1,
        state.editing_field == Some(WizardField::OpenCodePath),
        &state.input_buffer,
    );
    render_path_field(
        frame,
        chunks[4],
        "Pi CLI",
        &state.pi_path,
        state.deps.pi_cli.found,
        state.focused_field == 2,
        state.editing_field == Some(WizardField::PiPath),
        &state.input_buffer,
    );
    render_path_field(
        frame,
        chunks[5],
        "Codex CLI",
        &state.codex_path,
        state.deps.codex_cli.found,
        state.focused_field == 3,
        state.editing_field == Some(WizardField::CodexPath),
        &state.input_buffer,
    );
    render_path_field(
        frame,
        chunks[6],
        "LazyGit",
        &state.lazygit_path,
        state.deps.lazygit.found,
        state.focused_field == 4,
        state.editing_field == Some(WizardField::LazygitPath),
        &state.input_buffer,
    );

    let hint = Paragraph::new(Span::styled(
        "↑/↓ select field · e edit · 1/2/3/4 default backend · →/Tab next",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(hint, chunks[7]);
}

/// Render a single "<label> Path: <status>" + editable value block.
#[allow(clippy::too_many_arguments)]
fn render_path_field(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    found: bool,
    focused: bool,
    editing: bool,
    input_buffer: &str,
) {
    let style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let shown = if editing {
        format!("{}█", input_buffer)
    } else {
        value.to_string()
    };
    let (status, status_style) = if found {
        ("✓", Style::default().fg(Color::Green))
    } else {
        ("○", Style::default().fg(Color::Yellow))
    };
    let block = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                format!("{} Path: ", label),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(status, status_style),
        ]),
        Line::from(vec![Span::styled("▸ ", style), Span::styled(shown, style)]),
    ]);
    frame.render_widget(block, area);
}

fn render_confirmation(frame: &mut Frame, area: Rect, state: &WizardState) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);

    let header = Paragraph::new("Summary of your configuration:")
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(header, chunks[0]);

    let summary = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Shell:       ", Style::default().fg(Color::DarkGray)),
            Span::raw(state.selected_shell()),
        ]),
        Line::from(vec![
            Span::styled("  Default AI:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                state.selected_backend.short_label(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Claude CLI:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(&state.claude_path),
        ]),
        Line::from(vec![
            Span::styled("  OpenCode:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(&state.opencode_path),
        ]),
        Line::from(vec![
            Span::styled("  Pi:          ", Style::default().fg(Color::DarkGray)),
            Span::raw(&state.pi_path),
        ]),
        Line::from(vec![
            Span::styled("  Codex:       ", Style::default().fg(Color::DarkGray)),
            Span::raw(&state.codex_path),
        ]),
        Line::from(vec![
            Span::styled("  LazyGit:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(&state.lazygit_path),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Config file: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                crate::config::get_config_path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "~/.config/ai-workbench/config.yaml".to_string()),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ]);
    frame.render_widget(summary, chunks[1]);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Press ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Green)),
        Span::styled(
            " to save configuration",
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(hint, chunks[2]);
}

fn render_complete(frame: &mut Frame, area: Rect) {
    let content = Paragraph::new(vec![
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("✓ ", Style::default().fg(Color::Green)),
            Span::styled(
                "Setup Complete!",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("Your configuration has been saved."),
        Line::from(""),
        Line::from("Press Enter to start using AI Workbench."),
    ])
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(content, area);
}

fn render_ssh_image_paste(frame: &mut Frame, area: Rect, state: &WizardState) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // header
        Constraint::Length(2), // banner
        Constraint::Length(3), // detection result
        Constraint::Length(8), // setup instructions
        Constraint::Length(3), // mark configured hint
        Constraint::Min(0),
    ])
    .split(area);

    // Heading
    let heading = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "SSH session detected — image paste needs a helper",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
    ])
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(heading, chunks[0]);

    // Explanation banner
    let banner = Paragraph::new(vec![Line::from(
        "Ctrl+V in the Claude pane cannot reach the upstream pasteboard over SSH. \
         The recommended bridge is cc-clip (https://github.com/ShunmeiCho/cc-clip).",
    )])
    .wrap(Wrap { trim: true })
    .style(Style::default().fg(Color::White));
    frame.render_widget(banner, chunks[1]);

    // cc-clip detection
    let (status_label, status_color) = match &state.cc_clip_path {
        Some(p) => (
            format!(" \u{2713} cc-clip detected: {}", p.display()),
            Color::Green,
        ),
        None => (
            " \u{26A0} cc-clip not on PATH — install via `cargo install cc-clip` on this host."
                .to_string(),
            Color::Yellow,
        ),
    };
    let detection = Paragraph::new(Line::from(Span::styled(
        status_label,
        Style::default()
            .fg(status_color)
            .add_modifier(Modifier::BOLD),
    )))
    .wrap(Wrap { trim: true });
    frame.render_widget(detection, chunks[2]);

    // Setup instructions (Mac side + remote side)
    let instructions = Paragraph::new(vec![
        Line::from(Span::styled(
            "Setup (one-time):",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  1. On your Mac:    brew install shunmeicho/tap/cc-clip"),
        Line::from("  2. On your Mac:    start the daemon (cc-clip-daemon &)"),
        Line::from("  3. ~/.ssh/config:  add `RemoteForward 9998 localhost:9998` for this host"),
        Line::from("  4. On this host:   cargo install cc-clip"),
        Line::from(""),
        Line::from(Span::styled(
            "Re-run --ssh-paste-diag to verify.",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .wrap(Wrap { trim: false });
    frame.render_widget(instructions, chunks[3]);

    // Mark-as-configured hint
    let marked_label = if state.ssh_image_paste_marked_configured {
        Span::styled(
            "[m] \u{2713} marked as configured — paste hint silenced",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            "[m] mark as configured  •  [Enter] continue without changes",
            Style::default().fg(Color::Cyan),
        )
    };
    let mark = Paragraph::new(Line::from(marked_label));
    frame.render_widget(mark, chunks[4]);
}

fn render_footer(frame: &mut Frame, popup_area: Rect, state: &WizardState) {
    let footer_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + popup_area.height - 2,
        popup_area.width - 2,
        1,
    );

    let nav_text = match state.step {
        WizardStep::Welcome => "[Esc] Skip  [Enter] Continue →",
        WizardStep::Complete => "[Enter] Start",
        _ => {
            if state.editing_field.is_some() {
                "[Esc] Cancel  [Enter] Confirm"
            } else {
                "← [Left] Back  [Enter] Continue →  [Esc] Cancel"
            }
        }
    };

    let footer = Paragraph::new(nav_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(footer, footer_area);
}
