//! Terminal UI colour themes for ZeroClaw.
//!
//! Two built-in themes are provided: `Dark` (default) and `Light`.
//! Theme colours map directly to ratatui [`Style`] values used by the
//! render functions in [`super::app`].

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

// ── ThemeVariant ─────────────────────────────────────────────────────────────

/// Built-in theme variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeVariant {
    /// Dark terminal background (default).
    #[default]
    Dark,
    /// Light terminal background.
    Light,
}

// ── Theme ────────────────────────────────────────────────────────────────────

/// A resolved set of ratatui [`Style`]s used throughout the TUI.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Border and title.
    pub border: Style,
    /// Status bar background.
    pub status_bar: Style,
    /// Active tab / selected item.
    pub highlight: Style,
    /// Normal body text.
    pub body: Style,
    /// Dim / secondary text.
    pub dim: Style,
    /// Error / alert text.
    pub error: Style,
    /// Success / healthy text.
    pub success: Style,
    /// Key-hint text (e.g. `[q] quit`).
    pub keyhint: Style,
}

impl Theme {
    /// Build the dark theme.
    pub fn dark() -> Self {
        Self {
            border: Style::default().fg(Color::Cyan),
            status_bar: Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            body: Style::default().fg(Color::White),
            dim: Style::default().fg(Color::DarkGray),
            error: Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
            success: Style::default().fg(Color::Green),
            keyhint: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        }
    }

    /// Build the light theme.
    pub fn light() -> Self {
        Self {
            border: Style::default().fg(Color::Blue),
            status_bar: Style::default()
                .bg(Color::LightBlue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            highlight: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            body: Style::default().fg(Color::Black),
            dim: Style::default().fg(Color::Gray),
            error: Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
            success: Style::default().fg(Color::DarkGray),
            keyhint: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        }
    }

    /// Resolve a [`Theme`] from a [`ThemeVariant`].
    pub fn from_variant(variant: ThemeVariant) -> Self {
        match variant {
            ThemeVariant::Dark => Self::dark(),
            ThemeVariant::Light => Self::light(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_variant_is_dark() {
        assert_eq!(ThemeVariant::default(), ThemeVariant::Dark);
    }

    #[test]
    fn dark_theme_has_cyan_border() {
        let t = Theme::dark();
        assert_eq!(t.border.fg, Some(Color::Cyan));
    }

    #[test]
    fn light_theme_has_blue_border() {
        let t = Theme::light();
        assert_eq!(t.border.fg, Some(Color::Blue));
    }

    #[test]
    fn from_variant_dark_matches_dark() {
        let a = Theme::from_variant(ThemeVariant::Dark);
        let b = Theme::dark();
        // Compare a representative field.
        assert_eq!(a.border.fg, b.border.fg);
    }

    #[test]
    fn from_variant_light_matches_light() {
        let a = Theme::from_variant(ThemeVariant::Light);
        let b = Theme::light();
        assert_eq!(a.border.fg, b.border.fg);
    }

    #[test]
    fn theme_variant_roundtrips_serde() {
        let v: ThemeVariant = serde_json::from_str("\"light\"").unwrap();
        assert_eq!(v, ThemeVariant::Light);
        let s = serde_json::to_string(&ThemeVariant::Dark).unwrap();
        assert_eq!(s, "\"dark\"");
    }

    #[test]
    fn error_style_uses_red() {
        let dark = Theme::dark();
        assert_eq!(dark.error.fg, Some(Color::Red));
        let light = Theme::light();
        assert_eq!(light.error.fg, Some(Color::Red));
    }

    #[test]
    fn highlight_is_bold() {
        let t = Theme::dark();
        assert!(t.highlight.add_modifier.contains(Modifier::BOLD));
    }
}
