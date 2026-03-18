//! Read-only config field browser pane.
//!
//! Displays key-value pairs from the active [`Config`] in a scrollable list.
//! Navigate with ↑/↓; no editing is performed (config changes require a
//! SIGHUP reload or restart).

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::config::Config;
use crate::tui::theme::Theme;
use super::Pane;

// ── ConfigPane ────────────────────────────────────────────────────────────────

/// Read-only config key-value browser.
pub struct ConfigPane {
    /// Pre-computed `(key, value)` display pairs.
    entries: Vec<(String, String)>,
    cursor: usize,
}

impl ConfigPane {
    /// Build a pane from the active config.
    pub fn from_config(config: &Config) -> Self {
        let entries = extract_entries(config);
        Self { entries, cursor: 0 }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entry at the current cursor (key, value).
    pub fn selected(&self) -> Option<(&str, &str)> {
        self.entries
            .get(self.cursor)
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

impl Pane for ConfigPane {
    fn label(&self) -> &str {
        "Config"
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if !self.entries.is_empty() && self.cursor < self.entries.len() - 1 {
                    self.cursor += 1;
                }
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
        let border_style = if focused { theme.highlight } else { theme.border };
        let block = Block::default()
            .title(" Config (read-only) ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, (k, v))| {
                let key_style = if i == self.cursor {
                    theme.highlight
                } else {
                    theme.dim
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {k:<28}", k = k), key_style),
                    Span::styled(v.clone(), theme.body),
                ]))
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(self.cursor));
        let list = List::new(items).block(block);
        frame.render_stateful_widget(list, area, &mut list_state);
    }
}

/// Extract display-safe key-value pairs from a [`Config`].
fn extract_entries(config: &Config) -> Vec<(String, String)> {
    vec![
        (
            "default_provider".into(),
            config
                .default_provider
                .as_deref()
                .unwrap_or("(not set)")
                .into(),
        ),
        (
            "default_model".into(),
            config
                .default_model
                .as_deref()
                .unwrap_or("(not set)")
                .into(),
        ),
        (
            "default_temperature".into(),
            format!("{:.2}", config.default_temperature),
        ),
        (
            "workspace_dir".into(),
            config.workspace_dir.display().to_string(),
        ),
        ("config_path".into(), config.config_path.display().to_string()),
        (
            "transcription.enabled".into(),
            config.transcription.enabled.to_string(),
        ),
        ("tts.enabled".into(), config.tts.enabled.to_string()),
        ("tts.provider".into(), config.tts.default_provider.clone()),
        (
            "heartbeat.enabled".into(),
            config.heartbeat.enabled.to_string(),
        ),
        ("cron.enabled".into(), config.cron.enabled.to_string()),
        (
            "gateway.require_pairing".into(),
            config.gateway.require_pairing.to_string(),
        ),
        ("nodes.enabled".into(), config.nodes.enabled.to_string()),
        ("nodes.mdns.enabled".into(), config.nodes.mdns.enabled.to_string()),
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn from_config_populates_entries() {
        let pane = ConfigPane::from_config(&Config::default());
        assert!(!pane.is_empty());
    }

    #[test]
    fn entries_include_provider() {
        let mut config = Config::default();
        config.default_provider = Some("anthropic".into());
        let pane = ConfigPane::from_config(&config);
        let found = pane
            .entries
            .iter()
            .any(|(k, v)| k == "default_provider" && v == "anthropic");
        assert!(found, "default_provider not found in entries");
    }

    #[test]
    fn down_moves_cursor() {
        let mut pane = ConfigPane::from_config(&Config::default());
        let initial = pane.cursor;
        pane.handle_key(key(KeyCode::Down));
        assert_eq!(pane.cursor, initial + 1);
    }

    #[test]
    fn up_does_not_underflow() {
        let mut pane = ConfigPane::from_config(&Config::default());
        pane.handle_key(key(KeyCode::Up));
        assert_eq!(pane.cursor, 0);
    }

    #[test]
    fn down_does_not_overflow() {
        let mut pane = ConfigPane::from_config(&Config::default());
        let max = pane.len();
        for _ in 0..max + 10 {
            pane.handle_key(key(KeyCode::Down));
        }
        assert!(pane.cursor < max);
    }

    #[test]
    fn selected_returns_entry_at_cursor() {
        let pane = ConfigPane::from_config(&Config::default());
        let (k, _) = pane.selected().unwrap();
        assert_eq!(k, "default_provider");
    }

    #[test]
    fn label_is_config() {
        let pane = ConfigPane::from_config(&Config::default());
        assert_eq!(pane.label(), "Config");
    }
}
