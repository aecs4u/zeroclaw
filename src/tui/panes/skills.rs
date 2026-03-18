//! Skills management pane.
//!
//! Lists installed skills (name + enabled status).  The user can navigate
//! with ↑/↓ and toggle the selected skill on/off with Space.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::tui::theme::Theme;
use super::Pane;

// ── SkillEntry ────────────────────────────────────────────────────────────────

/// A single skill shown in the pane.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

impl SkillEntry {
    pub fn new(name: impl Into<String>, description: impl Into<String>, enabled: bool) -> Self {
        Self { name: name.into(), description: description.into(), enabled }
    }
}

// ── SkillsPane ────────────────────────────────────────────────────────────────

/// Skills list pane with enable/disable toggle.
pub struct SkillsPane {
    skills: Vec<SkillEntry>,
    cursor: usize,
}

impl SkillsPane {
    pub fn new(skills: Vec<SkillEntry>) -> Self {
        Self { skills, cursor: 0 }
    }

    /// Number of skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Selected skill (if any).
    pub fn selected(&self) -> Option<&SkillEntry> {
        self.skills.get(self.cursor)
    }

    /// Toggle the enabled state of the currently selected skill.
    pub fn toggle_selected(&mut self) {
        if let Some(s) = self.skills.get_mut(self.cursor) {
            s.enabled = !s.enabled;
        }
    }
}

impl Pane for SkillsPane {
    fn label(&self) -> &str {
        "Skills"
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
                if !self.skills.is_empty() && self.cursor < self.skills.len() - 1 {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Char(' ') => {
                self.toggle_selected();
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
        let border_style = if focused { theme.highlight } else { theme.border };
        let block = Block::default()
            .title(format!(" {} ({}) ", self.label(), self.skills.len()))
            .borders(Borders::ALL)
            .border_style(border_style);

        if self.skills.is_empty() {
            let empty = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                " No skills installed",
                theme.dim,
            )))
            .block(block);
            frame.render_widget(empty, area);
            return;
        }

        let items: Vec<ListItem> = self
            .skills
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let status = if s.enabled { "✓" } else { "✗" };
                let status_style = if s.enabled { theme.success } else { theme.error };
                let label_style = if i == self.cursor { theme.highlight } else { theme.body };

                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {status} "), status_style),
                    Span::styled(s.name.clone(), label_style),
                    Span::styled(format!("  {}", s.description), theme.dim),
                ]))
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(self.cursor));
        let list = List::new(items).block(block);
        frame.render_stateful_widget(list, area, &mut list_state);
    }
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

    fn sample_skills() -> Vec<SkillEntry> {
        vec![
            SkillEntry::new("web-search", "Search the web", true),
            SkillEntry::new("code-exec", "Execute code", false),
            SkillEntry::new("file-ops", "File operations", true),
        ]
    }

    #[test]
    fn new_pane_has_correct_len() {
        let pane = SkillsPane::new(sample_skills());
        assert_eq!(pane.len(), 3);
        assert!(!pane.is_empty());
    }

    #[test]
    fn empty_pane() {
        let pane = SkillsPane::new(vec![]);
        assert!(pane.is_empty());
        assert!(pane.selected().is_none());
    }

    #[test]
    fn down_moves_cursor() {
        let mut pane = SkillsPane::new(sample_skills());
        pane.handle_key(key(KeyCode::Down));
        assert_eq!(pane.selected().unwrap().name, "code-exec");
    }

    #[test]
    fn up_does_not_underflow() {
        let mut pane = SkillsPane::new(sample_skills());
        pane.handle_key(key(KeyCode::Up));
        assert_eq!(pane.cursor, 0);
    }

    #[test]
    fn down_does_not_overflow() {
        let mut pane = SkillsPane::new(sample_skills());
        for _ in 0..10 {
            pane.handle_key(key(KeyCode::Down));
        }
        assert_eq!(pane.cursor, 2);
    }

    #[test]
    fn space_toggles_enabled() {
        let mut pane = SkillsPane::new(sample_skills());
        assert!(pane.selected().unwrap().enabled);
        pane.handle_key(key(KeyCode::Char(' ')));
        assert!(!pane.selected().unwrap().enabled);
    }

    #[test]
    fn space_on_disabled_enables() {
        let mut pane = SkillsPane::new(sample_skills());
        pane.handle_key(key(KeyCode::Down));
        assert!(!pane.selected().unwrap().enabled);
        pane.handle_key(key(KeyCode::Char(' ')));
        assert!(pane.selected().unwrap().enabled);
    }

    #[test]
    fn label_is_skills() {
        let pane = SkillsPane::new(vec![]);
        assert_eq!(pane.label(), "Skills");
    }
}
