//! Content panes for the ZeroClaw TUI.
//!
//! Each pane manages its own state and implements [`Pane`]:
//!
//! | Module       | Purpose                                           |
//! |--------------|---------------------------------------------------|
//! | [`messages`] | Scrollable conversation history with inline markdown |
//! | [`skills`]   | Installed skills list with enable/disable toggle  |
//! | [`config`]   | Read-only config field browser                    |
//!
//! The active pane receives keyboard events; inactive panes only render.

pub mod config;
pub mod messages;
pub mod skills;

use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::tui::theme::Theme;

/// Common interface for TUI panes.
pub trait Pane {
    /// Human-readable label shown in the tab bar.
    fn label(&self) -> &str;

    /// Handle a key event when this pane is active. Returns `true` when consumed.
    fn handle_key(&mut self, key: KeyEvent) -> bool;

    /// Render pane contents into `area`.
    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool);
}
