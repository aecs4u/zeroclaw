//! Modal dialogs for the ZeroClaw TUI.
//!
//! Each dialog is a self-contained struct that can render itself as an overlay
//! on top of the main canvas via [`Dialog::render`], and advance its own state
//! through [`Dialog::handle_key`].
//!
//! # Available dialogs
//!
//! | Module            | Purpose                                         |
//! |-------------------|-------------------------------------------------|
//! | [`api_key`]       | Masked API key entry with provider validation   |
//! | [`model_selector`]| Filterable provider / model picker              |
//! | [`vault`]         | List and reveal secrets with confirmation       |
//!
//! # Usage
//!
//! ```rust,ignore
//! use zeroclaw::tui::dialogs::api_key::ApiKeyDialog;
//!
//! let mut dlg = ApiKeyDialog::new("openai");
//! // … pass key events from the main loop, render on each tick
//! ```

pub mod api_key;
pub mod model_selector;
pub mod vault;

use crossterm::event::KeyEvent;
use ratatui::Frame;

use crate::tui::theme::Theme;

/// Common interface implemented by all TUI dialogs.
pub trait Dialog {
    /// Whether the user has confirmed or cancelled so the caller can close.
    fn is_done(&self) -> bool;

    /// Handle a key event.  Returns `true` when the event was consumed.
    fn handle_key(&mut self, key: KeyEvent) -> bool;

    /// Render the dialog as a centred overlay.
    fn render(&self, frame: &mut Frame, theme: &Theme);
}

// ── Layout helpers ────────────────────────────────────────────────────────────

use ratatui::layout::Rect;

/// Centre a rectangle of `(width, height)` inside `area`.
pub(crate) fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centred_fits_inside_area() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centred(area, 40, 10);
        assert_eq!(r.width, 40);
        assert_eq!(r.height, 10);
        assert!(r.x + r.width <= area.x + area.width);
        assert!(r.y + r.height <= area.y + area.height);
    }

    #[test]
    fn centred_clamps_to_area() {
        let area = Rect::new(0, 0, 10, 5);
        let r = centred(area, 200, 200);
        assert_eq!(r.width, 10);
        assert_eq!(r.height, 5);
    }

    #[test]
    fn centred_horizontally() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centred(area, 40, 10);
        assert_eq!(r.x, 20); // (80 - 40) / 2
    }

    #[test]
    fn centred_vertically() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centred(area, 40, 10);
        assert_eq!(r.y, 7); // (24 - 10) / 2
    }
}
