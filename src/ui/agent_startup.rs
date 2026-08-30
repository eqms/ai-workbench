//! Claude-style, sectioned startup forms for non-Claude AI CLIs.

use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::backend::AiBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StartupProfile {
    pub name: &'static str,
    pub description: &'static str,
    pub args: &'static [&'static str],
    pub dangerous: bool,
}

/// Sections in the non-Claude startup forms. Like the Claude dialog, Tab and
/// Shift+Tab move between sections; arrows change the active value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSection {
    Mode,
    Sandbox,
    Approval,
    Model,
    Search,
    Session,
    Agent,
    Interface,
    Effort,
    Thinking,
    Tools,
    Offline,
}

const CODEX_SECTIONS: &[AgentSection] = &[
    AgentSection::Sandbox,
    AgentSection::Approval,
    AgentSection::Model,
    AgentSection::Search,
];
const OPENCODE_SECTIONS: &[AgentSection] = &[
    AgentSection::Session,
    AgentSection::Model,
    AgentSection::Agent,
    AgentSection::Interface,
    AgentSection::Approval,
];
const PI_SECTIONS: &[AgentSection] = &[
    AgentSection::Session,
    AgentSection::Model,
    AgentSection::Thinking,
    AgentSection::Tools,
    AgentSection::Offline,
];
const ANTIGRAVITY_SECTIONS: &[AgentSection] = &[
    AgentSection::Mode,
    AgentSection::Model,
    AgentSection::Agent,
    AgentSection::Effort,
    AgentSection::Session,
];
const PLAIN_SECTIONS: &[AgentSection] = &[AgentSection::Mode];

fn sections_for(backend: AiBackend) -> &'static [AgentSection] {
    match backend {
        AiBackend::Codex => CODEX_SECTIONS,
        AiBackend::OpenCode => OPENCODE_SECTIONS,
        AiBackend::Pi => PI_SECTIONS,
        AiBackend::Antigravity => ANTIGRAVITY_SECTIONS,
        _ => PLAIN_SECTIONS,
    }
}

const CODEX_SANDBOX: &[StartupProfile] = &[
    StartupProfile {
        name: "Config",
        description: "Sandbox aus ~/.codex/config.toml verwenden",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Workspace",
        description: "Im Workspace lesen und schreiben",
        args: &["--sandbox", "workspace-write"],
        dangerous: false,
    },
    StartupProfile {
        name: "Read-only",
        description: "Nur analysieren; keine Dateien verändern",
        args: &["--sandbox", "read-only"],
        dangerous: false,
    },
    StartupProfile {
        name: "Full access",
        description: "Keine Dateisystem-Sandbox",
        args: &["--sandbox", "danger-full-access"],
        dangerous: true,
    },
];
const CODEX_APPROVAL: &[StartupProfile] = &[
    StartupProfile {
        name: "Config",
        description: "Approval-Regel aus der Codex-Konfiguration",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "On request",
        description: "Codex entscheidet, wann eine Freigabe nötig ist",
        args: &["--ask-for-approval", "on-request"],
        dangerous: false,
    },
    StartupProfile {
        name: "Never",
        description: "Keine Rückfragen; Fehler gehen direkt ans Modell",
        args: &["--ask-for-approval", "never"],
        dangerous: true,
    },
];
const SESSION_CHOICES: &[StartupProfile] = &[
    StartupProfile {
        name: "New",
        description: "Neue Sitzung starten",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Continue",
        description: "Die zuletzt verwendete Sitzung fortsetzen",
        args: &["--continue"],
        dangerous: false,
    },
    StartupProfile {
        name: "Resume",
        description: "Interaktive Sitzungs-Auswahl öffnen",
        args: &["--resume"],
        dangerous: false,
    },
];
const OPENCODE_SESSION: &[StartupProfile] = &[
    StartupProfile {
        name: "New",
        description: "Neue OpenCode-Sitzung starten",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Continue",
        description: "Die letzte OpenCode-Sitzung fortsetzen",
        args: &["--continue"],
        dangerous: false,
    },
    StartupProfile {
        name: "Fork last",
        description: "Letzte Sitzung fortsetzen und als Fork öffnen",
        args: &["--continue", "--fork"],
        dangerous: false,
    },
];
const INTERFACE_CHOICES: &[StartupProfile] = &[
    StartupProfile {
        name: "Full TUI",
        description: "Vollständige OpenCode-Oberfläche",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Mini",
        description: "Minimale interaktive Oberfläche",
        args: &["--mini"],
        dangerous: false,
    },
];
const OPENCODE_APPROVAL: &[StartupProfile] = &[
    StartupProfile {
        name: "Normal",
        description: "Konfigurierte Berechtigungsregeln verwenden",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Auto",
        description: "Nicht explizit verbotene Berechtigungen automatisch freigeben",
        args: &["--auto"],
        dangerous: true,
    },
];
const THINKING_CHOICES: &[StartupProfile] = &[
    StartupProfile {
        name: "Default",
        description: "Thinking-Level aus der Pi-Konfiguration",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Off",
        description: "Kein erweitertes Reasoning",
        args: &["--thinking", "off"],
        dangerous: false,
    },
    StartupProfile {
        name: "Low",
        description: "Kurzes Reasoning",
        args: &["--thinking", "low"],
        dangerous: false,
    },
    StartupProfile {
        name: "Medium",
        description: "Ausgewogenes Reasoning",
        args: &["--thinking", "medium"],
        dangerous: false,
    },
    StartupProfile {
        name: "High",
        description: "Intensives Reasoning",
        args: &["--thinking", "high"],
        dangerous: false,
    },
    StartupProfile {
        name: "XHigh",
        description: "Sehr intensives Reasoning",
        args: &["--thinking", "xhigh"],
        dangerous: false,
    },
    StartupProfile {
        name: "Max",
        description: "Maximales Reasoning",
        args: &["--thinking", "max"],
        dangerous: false,
    },
];
const TOOLS_CHOICES: &[StartupProfile] = &[
    StartupProfile {
        name: "All",
        description: "Alle konfigurierten Werkzeuge aktivieren",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Read-only",
        description: "Nur read, grep, find und ls",
        args: &["--tools", "read,grep,find,ls"],
        dangerous: false,
    },
    StartupProfile {
        name: "None",
        description: "Ohne Werkzeuge starten",
        args: &["--no-tools"],
        dangerous: false,
    },
];
const ANTIGRAVITY_MODE: &[StartupProfile] = &[
    StartupProfile {
        name: "Default",
        description: "Konfigurierte Berechtigungen verwenden",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Plan",
        description: "Planen, ohne Änderungen auszuführen",
        args: &["--mode", "plan"],
        dangerous: false,
    },
    StartupProfile {
        name: "Accept edits",
        description: "Dateiänderungen akzeptieren; andere Aktionen weiter bestätigen",
        args: &["--mode", "accept-edits"],
        dangerous: false,
    },
    StartupProfile {
        name: "Sandbox",
        description: "Terminal-Aktionen zusätzlich einschränken",
        args: &["--sandbox"],
        dangerous: false,
    },
    StartupProfile {
        name: "YOLO",
        description: "Alle Tool-Freigaben überspringen",
        args: &["--dangerously-skip-permissions"],
        dangerous: true,
    },
];
const EFFORT_CHOICES: &[StartupProfile] = &[
    StartupProfile {
        name: "Default",
        description: "Effort aus der Antigravity-Konfiguration",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Low",
        description: "Schnelle, knappe Bearbeitung",
        args: &["--effort", "low"],
        dangerous: false,
    },
    StartupProfile {
        name: "Medium",
        description: "Ausgewogene Bearbeitung",
        args: &["--effort", "medium"],
        dangerous: false,
    },
    StartupProfile {
        name: "High",
        description: "Gründliche Bearbeitung",
        args: &["--effort", "high"],
        dangerous: false,
    },
];

const ANTIGRAVITY_SESSION: &[StartupProfile] = &[
    StartupProfile {
        name: "New",
        description: "Neue Antigravity-Konversation starten",
        args: &[],
        dangerous: false,
    },
    StartupProfile {
        name: "Continue",
        description: "Die zuletzt verwendete Konversation fortsetzen",
        args: &["--continue"],
        dangerous: false,
    },
];

#[derive(Debug, Clone)]
pub struct AgentStartupState {
    pub visible: bool,
    pub backend: AiBackend,
    pub section_index: usize,
    sandbox_selected: usize,
    approval_selected: usize,
    session_selected: usize,
    interface_selected: usize,
    thinking_selected: usize,
    tools_selected: usize,
    mode_selected: usize,
    effort_selected: usize,
    model: String,
    model_cursor: usize,
    agent: String,
    agent_cursor: usize,
    search_enabled: bool,
    offline_enabled: bool,
}

impl Default for AgentStartupState {
    fn default() -> Self {
        Self {
            visible: false,
            backend: AiBackend::default(),
            section_index: 0,
            sandbox_selected: 0,
            approval_selected: 0,
            session_selected: 0,
            interface_selected: 0,
            thinking_selected: 0,
            tools_selected: 0,
            mode_selected: 0,
            effort_selected: 0,
            model: String::new(),
            model_cursor: 0,
            agent: String::new(),
            agent_cursor: 0,
            search_enabled: false,
            offline_enabled: false,
        }
    }
}

impl AgentStartupState {
    pub fn open(&mut self, backend: AiBackend) {
        self.visible = true;
        self.backend = backend;
        self.section_index = 0;
        self.sandbox_selected = 0;
        self.approval_selected = 0;
        self.session_selected = 0;
        self.interface_selected = 0;
        self.thinking_selected = 0;
        self.tools_selected = 0;
        self.mode_selected = 0;
        self.effort_selected = 0;
        self.model.clear();
        self.model_cursor = 0;
        self.agent.clear();
        self.agent_cursor = 0;
        self.search_enabled = false;
        self.offline_enabled = false;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn next_section(&mut self) {
        let len = sections_for(self.backend).len();
        self.section_index = (self.section_index + 1) % len;
    }

    pub fn prev_section(&mut self) {
        let len = sections_for(self.backend).len();
        self.section_index = (self.section_index + len - 1) % len;
    }

    pub fn active_section(&self) -> AgentSection {
        sections_for(self.backend)
            .get(self.section_index)
            .copied()
            .unwrap_or(AgentSection::Mode)
    }

    pub fn is_text_field_active(&self) -> bool {
        matches!(
            self.active_section(),
            AgentSection::Model | AgentSection::Agent
        )
    }

    pub fn next(&mut self) {
        self.change_active_choice(1);
    }

    pub fn prev(&mut self) {
        self.change_active_choice(-1);
    }

    fn change_active_choice(&mut self, delta: isize) {
        let (selected, len) = match self.active_section() {
            AgentSection::Sandbox => (&mut self.sandbox_selected, CODEX_SANDBOX.len()),
            AgentSection::Approval if self.backend == AiBackend::Codex => {
                (&mut self.approval_selected, CODEX_APPROVAL.len())
            }
            AgentSection::Approval => (&mut self.approval_selected, OPENCODE_APPROVAL.len()),
            AgentSection::Session if self.backend == AiBackend::OpenCode => {
                (&mut self.session_selected, OPENCODE_SESSION.len())
            }
            AgentSection::Session if self.backend == AiBackend::Antigravity => {
                (&mut self.session_selected, ANTIGRAVITY_SESSION.len())
            }
            AgentSection::Session => (&mut self.session_selected, SESSION_CHOICES.len()),
            AgentSection::Interface => (&mut self.interface_selected, INTERFACE_CHOICES.len()),
            AgentSection::Thinking => (&mut self.thinking_selected, THINKING_CHOICES.len()),
            AgentSection::Tools => (&mut self.tools_selected, TOOLS_CHOICES.len()),
            AgentSection::Mode if self.backend == AiBackend::Antigravity => {
                (&mut self.mode_selected, ANTIGRAVITY_MODE.len())
            }
            AgentSection::Effort => (&mut self.effort_selected, EFFORT_CHOICES.len()),
            AgentSection::Search => {
                self.search_enabled = !self.search_enabled;
                return;
            }
            AgentSection::Offline => {
                self.offline_enabled = !self.offline_enabled;
                return;
            }
            _ => return,
        };
        *selected = ((*selected as isize + delta).rem_euclid(len as isize)) as usize;
    }

    pub fn insert_char(&mut self, character: char) {
        let (value, cursor) = match self.active_section() {
            AgentSection::Model => (&mut self.model, &mut self.model_cursor),
            AgentSection::Agent => (&mut self.agent, &mut self.agent_cursor),
            _ => return,
        };
        let mut chars = value.chars().collect::<Vec<_>>();
        chars.insert((*cursor).min(chars.len()), character);
        *cursor = cursor.saturating_add(1);
        *value = chars.into_iter().collect();
    }

    pub fn backspace(&mut self) {
        let (value, cursor) = match self.active_section() {
            AgentSection::Model => (&mut self.model, &mut self.model_cursor),
            AgentSection::Agent => (&mut self.agent, &mut self.agent_cursor),
            _ => return,
        };
        if *cursor == 0 {
            return;
        }
        let mut chars = value.chars().collect::<Vec<_>>();
        *cursor -= 1;
        chars.remove(*cursor);
        *value = chars.into_iter().collect();
    }

    pub fn delete(&mut self) {
        let (value, cursor) = match self.active_section() {
            AgentSection::Model => (&mut self.model, self.model_cursor),
            AgentSection::Agent => (&mut self.agent, self.agent_cursor),
            _ => return,
        };
        let mut chars = value.chars().collect::<Vec<_>>();
        if cursor < chars.len() {
            chars.remove(cursor);
            *value = chars.into_iter().collect();
        }
    }

    pub fn cursor_left(&mut self) {
        match self.active_section() {
            AgentSection::Model => self.model_cursor = self.model_cursor.saturating_sub(1),
            AgentSection::Agent => self.agent_cursor = self.agent_cursor.saturating_sub(1),
            _ => {}
        }
    }

    pub fn cursor_right(&mut self) {
        match self.active_section() {
            AgentSection::Model => {
                self.model_cursor = (self.model_cursor + 1).min(self.model.chars().count())
            }
            AgentSection::Agent => {
                self.agent_cursor = (self.agent_cursor + 1).min(self.agent.chars().count())
            }
            _ => {}
        }
    }

    pub fn cursor_home(&mut self) {
        match self.active_section() {
            AgentSection::Model => self.model_cursor = 0,
            AgentSection::Agent => self.agent_cursor = 0,
            _ => {}
        }
    }

    pub fn cursor_end(&mut self) {
        match self.active_section() {
            AgentSection::Model => self.model_cursor = self.model.chars().count(),
            AgentSection::Agent => self.agent_cursor = self.agent.chars().count(),
            _ => {}
        }
    }

    pub fn selected_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        match self.backend {
            AiBackend::Codex => {
                append_profile_args(&mut args, CODEX_SANDBOX[self.sandbox_selected]);
                append_profile_args(&mut args, CODEX_APPROVAL[self.approval_selected]);
                append_text_arg(&mut args, "--model", &self.model);
                if self.search_enabled {
                    args.push("--search".into());
                }
            }
            AiBackend::OpenCode => {
                append_profile_args(&mut args, OPENCODE_SESSION[self.session_selected]);
                append_text_arg(&mut args, "--model", &self.model);
                append_text_arg(&mut args, "--agent", &self.agent);
                append_profile_args(&mut args, INTERFACE_CHOICES[self.interface_selected]);
                append_profile_args(&mut args, OPENCODE_APPROVAL[self.approval_selected]);
            }
            AiBackend::Pi => {
                append_profile_args(&mut args, SESSION_CHOICES[self.session_selected]);
                append_text_arg(&mut args, "--model", &self.model);
                append_profile_args(&mut args, THINKING_CHOICES[self.thinking_selected]);
                append_profile_args(&mut args, TOOLS_CHOICES[self.tools_selected]);
                if self.offline_enabled {
                    args.push("--offline".into());
                }
            }
            AiBackend::Antigravity => {
                append_profile_args(&mut args, ANTIGRAVITY_MODE[self.mode_selected]);
                append_text_arg(&mut args, "--model", &self.model);
                append_text_arg(&mut args, "--agent", &self.agent);
                append_profile_args(&mut args, EFFORT_CHOICES[self.effort_selected]);
                append_profile_args(&mut args, ANTIGRAVITY_SESSION[self.session_selected]);
            }
            _ => {}
        }
        args
    }
}

fn append_profile_args(args: &mut Vec<String>, profile: StartupProfile) {
    args.extend(profile.args.iter().map(|arg| (*arg).to_string()));
}

fn append_text_arg(args: &mut Vec<String>, flag: &str, value: &str) {
    if !value.trim().is_empty() {
        args.push(flag.to_string());
        args.push(value.trim().to_string());
    }
}

fn choices_for(
    state: &AgentStartupState,
    section: AgentSection,
) -> Option<(&'static [StartupProfile], usize)> {
    match section {
        AgentSection::Sandbox => Some((CODEX_SANDBOX, state.sandbox_selected)),
        AgentSection::Approval if state.backend == AiBackend::Codex => {
            Some((CODEX_APPROVAL, state.approval_selected))
        }
        AgentSection::Approval => Some((OPENCODE_APPROVAL, state.approval_selected)),
        AgentSection::Session if state.backend == AiBackend::OpenCode => {
            Some((OPENCODE_SESSION, state.session_selected))
        }
        AgentSection::Session if state.backend == AiBackend::Antigravity => {
            Some((ANTIGRAVITY_SESSION, state.session_selected))
        }
        AgentSection::Session => Some((SESSION_CHOICES, state.session_selected)),
        AgentSection::Interface => Some((INTERFACE_CHOICES, state.interface_selected)),
        AgentSection::Thinking => Some((THINKING_CHOICES, state.thinking_selected)),
        AgentSection::Tools => Some((TOOLS_CHOICES, state.tools_selected)),
        AgentSection::Mode if state.backend == AiBackend::Antigravity => {
            Some((ANTIGRAVITY_MODE, state.mode_selected))
        }
        AgentSection::Effort => Some((EFFORT_CHOICES, state.effort_selected)),
        _ => None,
    }
}

fn section_title(section: AgentSection) -> &'static str {
    match section {
        AgentSection::Mode => "Mode",
        AgentSection::Sandbox => "Sandbox",
        AgentSection::Approval => "Approval",
        AgentSection::Model => "Model",
        AgentSection::Search => "Web Search",
        AgentSection::Session => "Session",
        AgentSection::Agent => "Agent",
        AgentSection::Interface => "Interface",
        AgentSection::Effort => "Effort",
        AgentSection::Thinking => "Thinking",
        AgentSection::Tools => "Tools",
        AgentSection::Offline => "Network",
    }
}

fn text_field(value: &str, cursor: usize, active: bool) -> Line<'static> {
    if value.is_empty() && !active {
        return Line::styled(
            "  (CLI-/Config-Standard)",
            Style::default().fg(Color::DarkGray),
        );
    }
    let mut chars = value.chars().collect::<Vec<_>>();
    if active {
        chars.insert(cursor.min(chars.len()), '│');
    }
    Line::styled(
        format!("  {}", chars.into_iter().collect::<String>()),
        Style::default().fg(if active { Color::Yellow } else { Color::White }),
    )
}

fn render_section(
    frame: &mut Frame,
    area: Rect,
    state: &AgentStartupState,
    section: AgentSection,
    active: bool,
) {
    let title_color = if active {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    frame.render_widget(
        Paragraph::new(format!(
            "[ {} ]{}",
            section_title(section),
            if active { "  ←→ wählen" } else { "" }
        ))
        .style(Style::default().fg(title_color).add_modifier(if active {
            Modifier::BOLD
        } else {
            Modifier::empty()
        })),
        Rect::new(area.x, area.y, area.width, 1),
    );

    let control_area = Rect::new(area.x, area.y + 1, area.width, 1);
    let description_area = Rect::new(area.x + 2, area.y + 2, area.width.saturating_sub(2), 1);
    if let Some((choices, selected)) = choices_for(state, section) {
        let spans = choices
            .iter()
            .enumerate()
            .flat_map(|(index, choice)| {
                let selected_here = index == selected;
                let color = if selected_here && choice.dangerous {
                    Color::Red
                } else if selected_here {
                    Color::Yellow
                } else {
                    Color::Gray
                };
                vec![
                    Span::styled(
                        format!(
                            "({}) {}",
                            if selected_here { "•" } else { " " },
                            choice.name
                        ),
                        Style::default().fg(color).add_modifier(if selected_here {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                    Span::raw("  "),
                ]
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(Line::from(spans)), control_area);
        let choice = choices[selected];
        frame.render_widget(
            Paragraph::new(choice.description).style(Style::default().fg(if choice.dangerous {
                Color::Red
            } else {
                Color::Gray
            })),
            description_area,
        );
        return;
    }

    match section {
        AgentSection::Model => {
            frame.render_widget(
                text_field(&state.model, state.model_cursor, active),
                control_area,
            );
            frame.render_widget(
                Paragraph::new("Leer = Modell aus der CLI-Konfiguration")
                    .style(Style::default().fg(Color::Gray)),
                description_area,
            );
        }
        AgentSection::Agent => {
            frame.render_widget(
                text_field(&state.agent, state.agent_cursor, active),
                control_area,
            );
            frame.render_widget(
                Paragraph::new("Leer = Agent aus der CLI-Konfiguration")
                    .style(Style::default().fg(Color::Gray)),
                description_area,
            );
        }
        AgentSection::Search => render_toggle(
            frame,
            control_area,
            description_area,
            state.search_enabled,
            "Websuche beim Start aktivieren",
        ),
        AgentSection::Offline => render_toggle(
            frame,
            control_area,
            description_area,
            state.offline_enabled,
            "Startup-Netzwerkzugriffe deaktivieren",
        ),
        AgentSection::Mode => {
            frame.render_widget(
                Paragraph::new("  Start mit dem in Settings konfigurierten Kommando"),
                control_area,
            );
            frame.render_widget(
                Paragraph::new(
                    "Diese Ollama-Variante besitzt hier keine zusätzlichen Startoptionen.",
                )
                .style(Style::default().fg(Color::Gray)),
                description_area,
            );
        }
        _ => {}
    }
}

fn render_toggle(
    frame: &mut Frame,
    control_area: Rect,
    description_area: Rect,
    enabled: bool,
    description: &str,
) {
    let line = Line::from(vec![
        Span::styled(
            format!("({}) Aus", if enabled { " " } else { "•" }),
            Style::default().fg(if enabled { Color::Gray } else { Color::Yellow }),
        ),
        Span::raw("  "),
        Span::styled(
            format!("({}) An", if enabled { "•" } else { " " }),
            Style::default().fg(if enabled { Color::Yellow } else { Color::Gray }),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), control_area);
    frame.render_widget(
        Paragraph::new(description).style(Style::default().fg(Color::Gray)),
        description_area,
    );
}

pub fn render(frame: &mut Frame, area: Rect, state: &AgentStartupState) {
    if !state.visible {
        return;
    }
    let sections = sections_for(state.backend);
    let popup_width = area.width.saturating_sub(2).min(104);
    let desired_height = sections.len() as u16 * 3 + 6;
    let popup_height = desired_height.min(area.height.saturating_sub(2));
    let popup = Rect::new(
        area.x + area.width.saturating_sub(popup_width) / 2,
        area.y + area.height.saturating_sub(popup_height) / 2,
        popup_width,
        popup_height,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" {} Startup ", state.backend.short_label()))
        .title_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(block, popup);
    let inner = popup.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    frame.render_widget(
        Paragraph::new(format!(
            "{} Startup-Optionen — Tab wechselt Sektion:",
            state.backend.short_label()
        )),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    for (index, section) in sections.iter().copied().enumerate() {
        let y = inner.y + 1 + index as u16 * 3;
        if y + 2 >= inner.y + inner.height.saturating_sub(1) {
            break;
        }
        render_section(
            frame,
            Rect::new(inner.x, y, inner.width, 3),
            state,
            section,
            index == state.section_index,
        );
    }
    let footer_area = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width,
        1,
    );
    let footer = Line::from(vec![
        Span::styled(
            " Tab ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(" Sektion  "),
        Span::styled(
            " ←→ ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(" Wählen  "),
        Span::styled(
            " Enter ",
            Style::default().bg(Color::Yellow).fg(Color::Black),
        ),
        Span::raw(" Start  "),
        Span::styled(
            " Esc ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(" Defaults"),
    ]);
    frame.render_widget(
        Paragraph::new(footer).wrap(Wrap { trim: true }),
        footer_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_non_claude_backend_has_startup_sections() {
        for backend in AiBackend::all()
            .into_iter()
            .filter(|b| *b != AiBackend::Claude)
        {
            assert!(!sections_for(backend).is_empty());
        }
    }

    #[test]
    fn section_navigation_wraps() {
        let mut state = AgentStartupState::default();
        state.open(AiBackend::Codex);
        state.prev_section();
        assert_eq!(state.section_index, CODEX_SECTIONS.len() - 1);
        state.next_section();
        assert_eq!(state.section_index, 0);
    }

    #[test]
    fn pi_options_are_combined_into_owned_args() {
        let mut state = AgentStartupState::default();
        state.open(AiBackend::Pi);
        state.tools_selected = 1;
        state.offline_enabled = true;
        assert_eq!(
            state.selected_args(),
            ["--tools", "read,grep,find,ls", "--offline"]
        );
    }

    #[test]
    fn utf8_text_editing_uses_character_positions() {
        let mut state = AgentStartupState::default();
        state.open(AiBackend::Codex);
        state.section_index = 2;
        state.insert_char('ä');
        state.insert_char('i');
        state.cursor_left();
        state.backspace();
        assert_eq!(state.model, "i");
    }
}
