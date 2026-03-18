//! Conversation history pane with inline markdown rendering.
//!
//! Displays `(role, text)` message pairs.  Markdown formatting is rendered
//! with basic inline styles:
//! - `**bold**` / `__bold__` → [`ratatui::style::Modifier::BOLD`]
//! - `` `code` `` → dim/reversed
//! - `# Heading` lines → bold + newline padding
//!
//! Scroll with ↑/↓ or PgUp/PgDn.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::theme::Theme;
use super::Pane;

// ── Message ───────────────────────────────────────────────────────────────────

/// A single message in the conversation history.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub text: String,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: "user".into(), text: text.into() }
    }
    pub fn assistant(text: impl Into<String>) -> Self {
        Self { role: "assistant".into(), text: text.into() }
    }
    pub fn system(text: impl Into<String>) -> Self {
        Self { role: "system".into(), text: text.into() }
    }
}

// ── MessagesPane ──────────────────────────────────────────────────────────────

/// Scrollable conversation history pane.
pub struct MessagesPane {
    messages: Vec<Message>,
    /// Line-scroll offset (0 = bottom / most recent).
    scroll: usize,
}

impl MessagesPane {
    pub fn new() -> Self {
        Self { messages: Vec::new(), scroll: 0 }
    }

    /// Append a message and reset scroll to bottom.
    pub fn push(&mut self, msg: Message) {
        self.messages.push(msg);
        self.scroll = 0;
    }

    /// Number of messages stored.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Build ratatui lines for all messages.
    pub fn render_lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        for msg in &self.messages {
            let role_style = match msg.role.as_str() {
                "user" => theme.highlight,
                "assistant" => theme.success,
                _ => theme.dim,
            };
            lines.push(Line::from(Span::styled(
                format!("[{}]", msg.role),
                role_style,
            )));
            lines.extend(render_markdown(&msg.text, theme));
            lines.push(Line::from(""));
        }
        lines
    }
}

impl Default for MessagesPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for MessagesPane {
    fn label(&self) -> &str {
        "Messages"
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_add(1);
                true
            }
            KeyCode::Down => {
                self.scroll = self.scroll.saturating_sub(1);
                true
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_add(10);
                true
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_sub(10);
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
        let border_style = if focused { theme.highlight } else { theme.border };
        let block = Block::default()
            .title(format!(" {} ", self.label()))
            .borders(Borders::ALL)
            .border_style(border_style);

        let lines = self.render_lines(theme);
        let total: u16 = lines.len().try_into().unwrap_or(u16::MAX);
        let inner_h = area.height.saturating_sub(2);
        let scroll: u16 = self
            .scroll
            .min(total.saturating_sub(inner_h) as usize)
            .try_into()
            .unwrap_or(u16::MAX);

        let para = Paragraph::new(lines)
            .block(block)
            .style(theme.body)
            .wrap(Wrap { trim: false })
            .scroll((total.saturating_sub(inner_h).saturating_sub(scroll), 0));

        frame.render_widget(para, area);
    }
}

// ── Inline markdown renderer ──────────────────────────────────────────────────

/// Convert `text` to ratatui [`Line`]s with basic inline markdown styles.
pub fn render_markdown(text: &str, theme: &Theme) -> Vec<Line<'static>> {
    text.lines().map(|line| render_line(line, theme)).collect()
}

fn render_line(line: &str, theme: &Theme) -> Line<'static> {
    // Heading
    if let Some(rest) = line.strip_prefix("# ") {
        return Line::from(Span::styled(
            rest.to_string(),
            theme.highlight.add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(rest) = line.strip_prefix("## ") {
        return Line::from(Span::styled(
            rest.to_string(),
            theme.highlight.add_modifier(Modifier::BOLD),
        ));
    }

    // Inline spans
    let mut spans = Vec::new();
    let mut remaining = line;

    while !remaining.is_empty() {
        // Bold: **text** or __text__
        if let Some((pre, rest)) = find_delim(remaining, "**") {
            if !pre.is_empty() {
                spans.push(Span::styled(pre.to_string(), theme.body));
            }
            if let Some((bold, after)) = find_delim(rest, "**") {
                spans.push(Span::styled(
                    bold.to_string(),
                    theme.body.add_modifier(Modifier::BOLD),
                ));
                remaining = after;
                continue;
            }
        }

        // Inline code: `text`
        if let Some((pre, rest)) = find_delim(remaining, "`") {
            if !pre.is_empty() {
                spans.push(Span::styled(pre.to_string(), theme.body));
            }
            if let Some((code, after)) = find_delim(rest, "`") {
                spans.push(Span::styled(
                    format!("`{code}`"),
                    theme.dim.add_modifier(Modifier::REVERSED),
                ));
                remaining = after;
                continue;
            }
        }

        // No match — output whole rest as plain text
        spans.push(Span::styled(remaining.to_string(), theme.body));
        break;
    }

    Line::from(spans)
}

/// Find the first occurrence of `delim` in `s`, returning `(before, after)`.
fn find_delim<'a>(s: &'a str, delim: &str) -> Option<(&'a str, &'a str)> {
    let idx = s.find(delim)?;
    Some((&s[..idx], &s[idx + delim.len()..]))
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

    fn theme() -> Theme {
        Theme::dark()
    }

    #[test]
    fn starts_empty() {
        let pane = MessagesPane::new();
        assert!(pane.is_empty());
        assert_eq!(pane.len(), 0);
    }

    #[test]
    fn push_increments_len() {
        let mut pane = MessagesPane::new();
        pane.push(Message::user("hello"));
        assert_eq!(pane.len(), 1);
        pane.push(Message::assistant("world"));
        assert_eq!(pane.len(), 2);
    }

    #[test]
    fn push_resets_scroll() {
        let mut pane = MessagesPane::new();
        pane.scroll = 5;
        pane.push(Message::user("new"));
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn scroll_up_increments() {
        let mut pane = MessagesPane::new();
        pane.handle_key(key(KeyCode::Up));
        assert_eq!(pane.scroll, 1);
    }

    #[test]
    fn scroll_down_does_not_underflow() {
        let mut pane = MessagesPane::new();
        pane.handle_key(key(KeyCode::Down));
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn render_lines_includes_role_labels() {
        let mut pane = MessagesPane::new();
        pane.push(Message::user("hi"));
        let lines = pane.render_lines(&theme());
        let text: String = lines.iter().flat_map(|l| l.spans.iter().map(|s| s.content.as_ref())).collect();
        assert!(text.contains("[user]"));
    }

    #[test]
    fn markdown_heading_renders() {
        let t = theme();
        let lines = render_markdown("# Title", &t);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans[0].content.contains("Title"));
        assert!(lines[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn markdown_plain_renders_as_body() {
        let t = theme();
        let lines = render_markdown("plain text", &t);
        assert_eq!(lines[0].spans[0].content.as_ref(), "plain text");
    }

    #[test]
    fn find_delim_splits_on_first_occurrence() {
        let (before, after) = find_delim("hello**world**", "**").unwrap();
        assert_eq!(before, "hello");
        assert_eq!(after, "world**");
    }

    #[test]
    fn find_delim_none_when_absent() {
        assert!(find_delim("no delimiter here", "**").is_none());
    }
}
