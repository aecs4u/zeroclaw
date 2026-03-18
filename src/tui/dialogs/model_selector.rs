//! Filterable provider / model selector dialog.
//!
//! Presents a scrollable list of `provider::model` entries that can be
//! narrowed by typing a filter string.
//!
//! ## Outcome
//!
//! After [`ModelSelectorDialog::is_done`] returns `true`:
//! - `Some((provider, model))` — user pressed Enter on a selection
//! - `None` — user pressed Escape (cancelled)

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::centred;
use crate::tui::theme::Theme;

// ── ModelSelectorDialog ────────────────────────────────────────────────────────

/// State for the model/provider selector modal.
pub struct ModelSelectorDialog {
    /// All entries as `(provider, model)` pairs.
    entries: Vec<(String, String)>,
    /// Current filter text.
    filter: String,
    /// Filtered indices into `entries`.
    filtered: Vec<usize>,
    /// Selected position within `filtered`.
    cursor: usize,
    /// `true` once Enter or Escape is pressed.
    done: bool,
    /// `Some((provider, model))` on confirm, `None` on cancel.
    pub outcome: Option<(String, String)>,
}

impl ModelSelectorDialog {
    /// Create a new dialog with the given `(provider, model)` entries.
    pub fn new(entries: Vec<(String, String)>) -> Self {
        let count = entries.len();
        Self {
            entries,
            filter: String::new(),
            filtered: (0..count).collect(),
            cursor: 0,
            done: false,
            outcome: None,
        }
    }

    /// Current filter string.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Number of items currently visible.
    pub fn visible_count(&self) -> usize {
        self.filtered.len()
    }

    fn apply_filter(&mut self) {
        let lower = self.filter.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, (p, m))| {
                let combined = format!("{p}::{m}").to_lowercase();
                combined.contains(&lower)
            })
            .map(|(i, _)| i)
            .collect();
        self.cursor = 0;
    }
}

impl super::Dialog for ModelSelectorDialog {
    fn is_done(&self) -> bool {
        self.done
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.apply_filter();
                true
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.apply_filter();
                true
            }
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if !self.filtered.is_empty() && self.cursor < self.filtered.len() - 1 {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Enter => {
                if let Some(&idx) = self.filtered.get(self.cursor) {
                    let (p, m) = &self.entries[idx];
                    self.outcome = Some((p.clone(), m.clone()));
                }
                self.done = true;
                true
            }
            KeyCode::Esc => {
                self.done = true;
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, theme: &Theme) {
        let area = frame.area();
        let popup = centred(area, 60, 20);

        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Select model ")
            .borders(Borders::ALL)
            .border_style(theme.highlight);

        // Filter input line
        let filter_line = Line::from(vec![
            Span::styled(" Filter: ", theme.dim),
            Span::styled(self.filter.as_str(), theme.body),
            Span::styled("█", theme.highlight), // cursor
        ]);

        let items: Vec<ListItem> = self
            .filtered
            .iter()
            .enumerate()
            .map(|(i, &idx)| {
                let (p, m) = &self.entries[idx];
                let label = format!("{p}  {m}");
                let style = if i == self.cursor {
                    theme.highlight
                } else {
                    theme.body
                };
                ListItem::new(Line::from(Span::styled(format!(" {label}"), style)))
            })
            .collect();

        // Render filter above, then list
        use ratatui::layout::{Constraint, Direction, Layout};
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // filter line
                Constraint::Length(1), // separator
                Constraint::Min(1),    // list
                Constraint::Length(1), // hint
            ])
            .split(inner);

        frame.render_widget(Paragraph::new(filter_line), layout[0]);

        let hint = Paragraph::new(Line::from(Span::styled(
            " ↑↓ navigate  [Enter] select  [Esc] cancel",
            theme.keyhint,
        )));
        frame.render_widget(hint, layout[3]);

        let mut list_state = ListState::default();
        list_state.select(Some(self.cursor));
        let list = List::new(items);
        frame.render_stateful_widget(list, layout[2], &mut list_state);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::dialogs::Dialog;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn entries() -> Vec<(String, String)> {
        vec![
            ("openai".into(), "gpt-4o".into()),
            ("openai".into(), "gpt-4o-mini".into()),
            ("anthropic".into(), "claude-opus-4".into()),
            ("anthropic".into(), "claude-sonnet-4".into()),
        ]
    }

    #[test]
    fn all_entries_visible_initially() {
        let dlg = ModelSelectorDialog::new(entries());
        assert_eq!(dlg.visible_count(), 4);
        assert!(!dlg.is_done());
    }

    #[test]
    fn filter_narrows_list() {
        let mut dlg = ModelSelectorDialog::new(entries());
        dlg.handle_key(key(KeyCode::Char('a')));
        dlg.handle_key(key(KeyCode::Char('n')));
        // "an" matches anthropic entries
        assert_eq!(dlg.visible_count(), 2);
    }

    #[test]
    fn backspace_widens_list() {
        let mut dlg = ModelSelectorDialog::new(entries());
        dlg.handle_key(key(KeyCode::Char('a')));
        dlg.handle_key(key(KeyCode::Char('n')));
        dlg.handle_key(key(KeyCode::Backspace));
        // back to one char — matches "a" in any field
        assert!(dlg.visible_count() >= 2);
    }

    #[test]
    fn enter_selects_current() {
        let mut dlg = ModelSelectorDialog::new(entries());
        dlg.handle_key(key(KeyCode::Enter));
        assert!(dlg.is_done());
        assert!(dlg.outcome.is_some());
        let (p, m) = dlg.outcome.unwrap();
        assert_eq!(p, "openai");
        assert_eq!(m, "gpt-4o");
    }

    #[test]
    fn down_moves_cursor() {
        let mut dlg = ModelSelectorDialog::new(entries());
        dlg.handle_key(key(KeyCode::Down));
        dlg.handle_key(key(KeyCode::Enter));
        let (_, m) = dlg.outcome.unwrap();
        assert_eq!(m, "gpt-4o-mini");
    }

    #[test]
    fn up_does_not_underflow() {
        let mut dlg = ModelSelectorDialog::new(entries());
        dlg.handle_key(key(KeyCode::Up)); // already at 0
        dlg.handle_key(key(KeyCode::Enter));
        let (p, _) = dlg.outcome.unwrap();
        assert_eq!(p, "openai"); // still first entry
    }

    #[test]
    fn escape_cancels() {
        let mut dlg = ModelSelectorDialog::new(entries());
        dlg.handle_key(key(KeyCode::Esc));
        assert!(dlg.is_done());
        assert!(dlg.outcome.is_none());
    }

    #[test]
    fn enter_on_empty_filter_result_still_terminates() {
        let mut dlg = ModelSelectorDialog::new(entries());
        // Filter to nothing
        for c in "zzz".chars() {
            dlg.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(dlg.visible_count(), 0);
        dlg.handle_key(key(KeyCode::Enter));
        assert!(dlg.is_done());
        assert!(dlg.outcome.is_none()); // no entry to select
    }
}
