//! Secrets vault viewer dialog.
//!
//! Lists secret names stored in the vault. The user can navigate the list
//! and press Enter to reveal a secret (the revealed value is shown for one
//! render tick, then hidden again).
//!
//! Actual secret decryption is performed by the caller — this dialog holds
//! only the list metadata and the outcome signal.
//!
//! ## Outcome
//!
//! After [`VaultDialog::is_done`] returns `true`:
//! - `Some(key)` — user selected a secret and confirmed reveal
//! - `None` — user pressed Escape (cancelled)

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::centred;
use crate::tui::theme::Theme;

// ── VaultDialog ────────────────────────────────────────────────────────────────

/// State for the vault secrets viewer modal.
pub struct VaultDialog {
    /// Secret names shown in the list.
    keys: Vec<String>,
    /// Current cursor position.
    cursor: usize,
    /// `true` once the dialog should close.
    done: bool,
    /// The selected key name on confirm, `None` on cancel.
    pub outcome: Option<String>,
    /// If `Some`, the revealed value (shown briefly by the caller).
    pub revealed: Option<String>,
}

impl VaultDialog {
    /// Create a new dialog with the provided secret `keys`.
    pub fn new(keys: Vec<String>) -> Self {
        Self {
            keys,
            cursor: 0,
            done: false,
            outcome: None,
            revealed: None,
        }
    }

    /// Set a revealed secret value for display (caller-provided after decryption).
    pub fn set_revealed(&mut self, value: impl Into<String>) {
        self.revealed = Some(value.into());
    }

    /// Selected key name (cursor position), if any entries exist.
    pub fn selected_key(&self) -> Option<&str> {
        self.keys.get(self.cursor).map(|s| s.as_str())
    }
}

impl super::Dialog for VaultDialog {
    fn is_done(&self) -> bool {
        self.done
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // If a value is being revealed, any key clears it.
        if self.revealed.is_some() {
            self.revealed = None;
            return true;
        }

        match key.code {
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if !self.keys.is_empty() && self.cursor < self.keys.len() - 1 {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Enter => {
                if let Some(k) = self.keys.get(self.cursor) {
                    self.outcome = Some(k.clone());
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
        let popup = centred(area, 60, 18);

        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Secrets vault ")
            .borders(Borders::ALL)
            .border_style(theme.highlight);

        use ratatui::layout::{Constraint, Direction, Layout};
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        // If revealing: show the value instead of the list
        if let Some(ref val) = self.revealed {
            let lines = vec![
                Line::from(""),
                Line::from(vec![Span::styled(" Revealed value:", theme.dim)]),
                Line::from(""),
                Line::from(vec![Span::styled(format!("  {val}"), theme.highlight)]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    " Press any key to hide.",
                    theme.keyhint,
                )]),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
            return;
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(inner);

        let items: Vec<ListItem> = self
            .keys
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let style = if i == self.cursor {
                    theme.highlight
                } else {
                    theme.body
                };
                ListItem::new(Line::from(Span::styled(format!(" {k}"), style)))
            })
            .collect();

        let hint = Paragraph::new(Line::from(Span::styled(
            " ↑↓ navigate  [Enter] reveal  [Esc] cancel",
            theme.keyhint,
        )));
        frame.render_widget(hint, layout[1]);

        if items.is_empty() {
            let empty = Paragraph::new(Line::from(Span::styled(
                " (vault is empty)",
                theme.dim,
            )));
            frame.render_widget(empty, layout[0]);
        } else {
            let mut list_state = ListState::default();
            list_state.select(Some(self.cursor));
            frame.render_stateful_widget(List::new(items), layout[0], &mut list_state);
        }
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

    fn dlg_with_keys() -> VaultDialog {
        VaultDialog::new(vec![
            "openai_api_key".into(),
            "telegram_bot_token".into(),
            "stripe_secret".into(),
        ])
    }

    #[test]
    fn starts_not_done() {
        let dlg = dlg_with_keys();
        assert!(!dlg.is_done());
        assert!(dlg.outcome.is_none());
    }

    #[test]
    fn enter_sets_outcome_to_selected_key() {
        let mut dlg = dlg_with_keys();
        dlg.handle_key(key(KeyCode::Enter));
        assert!(dlg.is_done());
        assert_eq!(dlg.outcome.as_deref(), Some("openai_api_key"));
    }

    #[test]
    fn down_moves_to_next_key() {
        let mut dlg = dlg_with_keys();
        dlg.handle_key(key(KeyCode::Down));
        dlg.handle_key(key(KeyCode::Enter));
        assert_eq!(dlg.outcome.as_deref(), Some("telegram_bot_token"));
    }

    #[test]
    fn up_does_not_underflow() {
        let mut dlg = dlg_with_keys();
        dlg.handle_key(key(KeyCode::Up));
        assert_eq!(dlg.selected_key(), Some("openai_api_key"));
    }

    #[test]
    fn down_does_not_overflow() {
        let mut dlg = dlg_with_keys();
        for _ in 0..10 {
            dlg.handle_key(key(KeyCode::Down));
        }
        assert_eq!(dlg.selected_key(), Some("stripe_secret"));
    }

    #[test]
    fn escape_cancels_without_outcome() {
        let mut dlg = dlg_with_keys();
        dlg.handle_key(key(KeyCode::Esc));
        assert!(dlg.is_done());
        assert!(dlg.outcome.is_none());
    }

    #[test]
    fn revealed_cleared_by_any_key() {
        let mut dlg = dlg_with_keys();
        dlg.set_revealed("sk-test123");
        assert!(dlg.revealed.is_some());
        dlg.handle_key(key(KeyCode::Char('x')));
        assert!(dlg.revealed.is_none());
    }

    #[test]
    fn empty_vault_enter_terminates_no_outcome() {
        let mut dlg = VaultDialog::new(vec![]);
        dlg.handle_key(key(KeyCode::Enter));
        assert!(dlg.is_done());
        assert!(dlg.outcome.is_none());
    }
}
