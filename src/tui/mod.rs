//! Terminal UI for ZeroClaw (optional `tui` feature).
//!
//! Enable with:
//!
//! ```bash
//! cargo build --features tui
//! zeroclaw tui
//! ```
//!
//! The TUI provides a split-pane terminal dashboard:
//! - **Status pane** — active provider, model, and daemon health at a glance.
//! - **Log pane** — scrolling tail of recent agent events.
//!
//! See [`app::run_tui`] for the entry point and [`theme::ThemeVariant`] for
//! available colour themes.

pub mod app;
pub mod dialogs;
pub mod theme;

pub use app::run_tui;
pub use theme::ThemeVariant;
