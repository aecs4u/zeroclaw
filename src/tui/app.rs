//! TUI application state and event loop.
//!
//! [`App`] holds all mutable state for the TUI session.
//! [`run_tui`] is the entry point called by the `zeroclaw tui` CLI command:
//! it initialises the terminal, runs the event loop, and restores the terminal
//! on exit (including on panic or error).
//!
//! ## Keybindings
//!
//! | Key          | Action             |
//! |------------- |--------------------|
//! | `q` / `Q`    | Quit               |
//! | `Ctrl-C`     | Quit               |
//! | `Tab`        | Next pane          |
//! | `Shift-Tab`  | Previous pane      |
//! | `?`          | Toggle help        |

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{io, time::Duration};

use crate::config::Config;
use super::theme::{Theme, ThemeVariant};

// ── Pane ──────────────────────────────────────────────────────────────────────

/// TUI pane identifiers (selectable with Tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Status,
    Log,
}

impl Pane {
    const ALL: &'static [Pane] = &[Pane::Status, Pane::Log];

    pub fn label(self) -> &'static str {
        match self {
            Pane::Status => "Status",
            Pane::Log => "Log",
        }
    }

    fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

/// Mutable TUI application state.
pub struct App {
    /// Whether the user has requested a quit.
    pub should_quit: bool,
    /// Whether the help overlay is visible.
    pub show_help: bool,
    /// Currently focused pane.
    pub active_pane: Pane,
    /// Resolved visual theme.
    pub theme: Theme,
    /// Lines shown in the Log pane (most-recent at end).
    pub log_lines: Vec<String>,
    /// Status summary text.
    pub status_text: String,
}

impl App {
    /// Create a new [`App`] from a resolved theme variant.
    pub fn new(variant: ThemeVariant) -> Self {
        Self {
            should_quit: false,
            show_help: false,
            active_pane: Pane::Status,
            theme: Theme::from_variant(variant),
            log_lines: Vec::new(),
            status_text: "Connecting…".to_string(),
        }
    }

    /// Handle a key event.  Returns `true` if the event was consumed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            // Quit
            (KeyCode::Char('q') | KeyCode::Char('Q'), _)
            | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.should_quit = true;
                true
            }
            // Help overlay
            (KeyCode::Char('?'), _) => {
                self.show_help = !self.show_help;
                true
            }
            // Tab navigation
            (KeyCode::Tab, _) => {
                self.active_pane = self.active_pane.next();
                true
            }
            (KeyCode::BackTab, _) => {
                self.active_pane = self.active_pane.prev();
                true
            }
            _ => false,
        }
    }

    /// Append a line to the log pane (keeps the last 1 000 lines).
    pub fn push_log(&mut self, line: impl Into<String>) {
        self.log_lines.push(line.into());
        if self.log_lines.len() > 1_000 {
            self.log_lines.drain(..self.log_lines.len() - 1_000);
        }
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Render the full TUI frame.
pub fn render(frame: &mut Frame, app: &App, _config: &Config) {
    let area = frame.area();

    // Split: top body, bottom status bar
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let body_area = outer[0];
    let bar_area = outer[1];

    // Split body into panes: Status | Log
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(body_area);

    render_status_pane(frame, app, panes[0]);
    render_log_pane(frame, app, panes[1]);
    render_status_bar(frame, app, bar_area);

    if app.show_help {
        render_help_overlay(frame, app, area);
    }
}

fn render_status_pane(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.active_pane == Pane::Status {
        app.theme.highlight
    } else {
        app.theme.border
    };

    let block = Block::default()
        .title(" Status ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let text = Paragraph::new(app.status_text.as_str())
        .style(app.theme.body)
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(text, area);
}

fn render_log_pane(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.active_pane == Pane::Log {
        app.theme.highlight
    } else {
        app.theme.border
    };

    let block = Block::default()
        .title(" Log ")
        .borders(Borders::ALL)
        .border_style(border_style);

    // Show the last N lines that fit in the pane
    let inner_height = area.height.saturating_sub(2) as usize; // subtract borders
    let start = app.log_lines.len().saturating_sub(inner_height);
    let visible: Vec<Line> = app.log_lines[start..]
        .iter()
        .map(|l| Line::from(l.as_str()))
        .collect();

    let text = Paragraph::new(visible)
        .style(app.theme.body)
        .block(block);

    frame.render_widget(text, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let pane_labels: Vec<Span> = Pane::ALL
        .iter()
        .flat_map(|p| {
            let style = if *p == app.active_pane {
                app.theme.highlight
            } else {
                app.theme.dim
            };
            [Span::styled(format!(" {} ", p.label()), style)]
        })
        .collect();

    let hints = Span::styled("  [Tab] pane  [?] help  [q] quit", app.theme.keyhint);
    let mut spans = pane_labels;
    spans.push(hints);

    let bar = Paragraph::new(Line::from(spans)).style(app.theme.status_bar);
    frame.render_widget(bar, area);
}

fn render_help_overlay(frame: &mut Frame, app: &App, area: Rect) {
    // Centre a 40×12 popup
    let popup_w = 44u16.min(area.width);
    let popup_h = 12u16.min(area.height);
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Keybindings ")
        .borders(Borders::ALL)
        .border_style(app.theme.highlight);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  q / Q / Ctrl-C  ", app.theme.keyhint),
            Span::styled("Quit", app.theme.body),
        ]),
        Line::from(vec![
            Span::styled("  Tab             ", app.theme.keyhint),
            Span::styled("Next pane", app.theme.body),
        ]),
        Line::from(vec![
            Span::styled("  Shift-Tab       ", app.theme.keyhint),
            Span::styled("Previous pane", app.theme.body),
        ]),
        Line::from(vec![
            Span::styled("  ?               ", app.theme.keyhint),
            Span::styled("Toggle this help", app.theme.body),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press any key to close", app.theme.dim),
        ]),
    ];

    let para = Paragraph::new(text).block(block);
    frame.render_widget(para, popup_area);
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Run the TUI event loop until the user quits.
///
/// Initialises a raw-mode alternate-screen terminal and restores it on return
/// (both normal and panic paths).
pub fn run_tui(config: &Config, theme: ThemeVariant) -> Result<()> {
    // Setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Ensure cleanup even on panic.
    let result = run_loop(&mut terminal, config, theme);

    // Teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    config: &Config,
    theme: ThemeVariant,
) -> Result<()> {
    let mut app = App::new(theme);
    app.push_log("ZeroClaw TUI started");
    app.status_text = format!(
        "Provider: {}\nModel:    {}",
        config.default_provider.as_deref().unwrap_or("(none)"),
        config.default_model.as_deref().unwrap_or("(none)"),
    );

    loop {
        terminal.draw(|f| render(f, &app, config))?;

        // Poll for events with a 100 ms timeout so the UI stays responsive.
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Close help overlay on any key
                if app.show_help {
                    app.show_help = false;
                } else {
                    app.handle_key(key);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
