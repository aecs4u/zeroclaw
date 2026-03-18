//! Masked API key entry dialog.
//!
//! Presents a single-line masked input field for entering an API key for a
//! named provider.  The entered text is always displayed as `•` characters
//! so the key is never visible on screen.
//!
//! ## Outcome
//!
//! After [`ApiKeyDialog::is_done`] returns `true`, inspect
//! [`ApiKeyDialog::outcome`]:
//! - `Some(key)` — user pressed Enter; `key` is the raw text
//! - `None` — user pressed Escape (cancelled)

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::centred;
use crate::tui::theme::Theme;

// ── ApiKeyDialog ───────────────────────────────────────────────────────────────

/// State for the API key entry modal.
pub struct ApiKeyDialog {
    /// Provider name shown in the dialog title.
    provider: String,
    /// Raw (unmasked) key text accumulated so far.
    buffer: String,
    /// `true` once Enter or Escape is pressed.
    done: bool,
    /// `Some(key)` on confirm, `None` on cancel.
    pub outcome: Option<String>,
    /// Optional validation error shown under the input field.
    error_msg: Option<String>,
}

impl ApiKeyDialog {
    /// Create a new dialog for `provider` (e.g. `"openai"`).
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            buffer: String::new(),
            done: false,
            outcome: None,
            error_msg: None,
        }
    }

    /// Raw key text accumulated so far (use for testing; not for display).
    #[cfg(test)]
    pub fn raw_text(&self) -> &str {
        &self.buffer
    }

    /// Set a validation error message shown under the input.
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error_msg = Some(msg.into());
    }

    /// Return the mask string (`•` repeated for each char).
    fn masked(&self) -> String {
        "•".repeat(self.buffer.chars().count())
    }
}

impl super::Dialog for ApiKeyDialog {
    fn is_done(&self) -> bool {
        self.done
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.buffer.push(c);
                self.error_msg = None;
                true
            }
            KeyCode::Backspace => {
                self.buffer.pop();
                self.error_msg = None;
                true
            }
            KeyCode::Enter => {
                if self.buffer.is_empty() {
                    self.error_msg = Some("API key must not be empty".into());
                } else {
                    self.outcome = Some(self.buffer.clone());
                    self.done = true;
                }
                true
            }
            KeyCode::Esc => {
                self.done = true; // outcome stays None → cancel
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, theme: &Theme) {
        let area = frame.area();
        let popup = centred(area, 52, if self.error_msg.is_some() { 7 } else { 6 });

        frame.render_widget(Clear, popup);

        let title = format!(" API key — {} ", self.provider);
        let block = Block::default()
            .title(title.as_str())
            .borders(Borders::ALL)
            .border_style(theme.highlight);

        let masked = self.masked();
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Key: ", theme.dim),
                Span::styled(masked, theme.body),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  [Enter] confirm  [Esc] cancel",
                theme.keyhint,
            )]),
        ];

        if let Some(ref err) = self.error_msg {
            lines.push(Line::from(vec![
                Span::styled("  ✗ ", theme.error),
                Span::styled(err.as_str(), theme.error),
            ]));
        }

        let para = Paragraph::new(lines).block(block);
        frame.render_widget(para, popup);
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

    #[test]
    fn starts_empty_and_not_done() {
        let dlg = ApiKeyDialog::new("openai");
        assert!(!dlg.is_done());
        assert!(dlg.outcome.is_none());
        assert_eq!(dlg.raw_text(), "");
    }

    #[test]
    fn typing_builds_buffer() {
        let mut dlg = ApiKeyDialog::new("openai");
        for c in "sk-test".chars() {
            dlg.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(dlg.raw_text(), "sk-test");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut dlg = ApiKeyDialog::new("openai");
        dlg.handle_key(key(KeyCode::Char('a')));
        dlg.handle_key(key(KeyCode::Char('b')));
        dlg.handle_key(key(KeyCode::Backspace));
        assert_eq!(dlg.raw_text(), "a");
    }

    #[test]
    fn enter_with_text_sets_outcome() {
        let mut dlg = ApiKeyDialog::new("openai");
        dlg.handle_key(key(KeyCode::Char('k')));
        dlg.handle_key(key(KeyCode::Enter));
        assert!(dlg.is_done());
        assert_eq!(dlg.outcome.as_deref(), Some("k"));
    }

    #[test]
    fn enter_empty_sets_error_not_done() {
        let mut dlg = ApiKeyDialog::new("openai");
        dlg.handle_key(key(KeyCode::Enter));
        assert!(!dlg.is_done());
        assert!(dlg.error_msg.is_some());
    }

    #[test]
    fn escape_cancels_without_outcome() {
        let mut dlg = ApiKeyDialog::new("openai");
        dlg.handle_key(key(KeyCode::Char('x')));
        dlg.handle_key(key(KeyCode::Esc));
        assert!(dlg.is_done());
        assert!(dlg.outcome.is_none());
    }

    #[test]
    fn masked_hides_characters() {
        let mut dlg = ApiKeyDialog::new("openai");
        dlg.handle_key(key(KeyCode::Char('s')));
        dlg.handle_key(key(KeyCode::Char('k')));
        assert_eq!(dlg.masked(), "••");
    }

    #[test]
    fn typing_clears_error() {
        let mut dlg = ApiKeyDialog::new("openai");
        dlg.handle_key(key(KeyCode::Enter)); // trigger error
        assert!(dlg.error_msg.is_some());
        dlg.handle_key(key(KeyCode::Char('a'))); // should clear error
        assert!(dlg.error_msg.is_none());
    }
}
