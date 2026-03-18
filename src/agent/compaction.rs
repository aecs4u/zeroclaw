//! Supplementary context-compaction strategies for the agent conversation loop.
//!
//! ZeroClaw's primary compaction is LLM-based (see `loop_::auto_compact_history`):
//! it summarises older turns with the active provider when the message count or
//! estimated-token budget is exceeded.
//!
//! This module adds **provider-independent** fallback strategies that run without
//! a network call or LLM inference:
//!
//! | Strategy       | How it works                                                  |
//! |----------------|---------------------------------------------------------------|
//! | `SlidingWindow`| Keep first N + last M messages; insert a tombstone in between |
//! | `Importance`   | Heuristic score each message; drop lowest-scoring middle turns |
//!
//! `Summarize` and `Hybrid` are intentionally not re-implemented here because
//! ZeroClaw's existing `auto_compact_history` already provides LLM summarisation.
//!
//! # Configuration
//!
//! ```toml
//! [agent.compaction]
//! strategy          = "sliding_window"   # "sliding_window" | "importance"
//! max_messages      = 100               # trigger threshold (non-system messages)
//! keep_initial      = 5                 # initial turns always kept
//! keep_recent       = 20                # most-recent turns always kept
//! importance_threshold = 0.5            # for "importance" strategy (0.0–1.0)
//! ```

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::providers::ChatMessage;

// ── Strategy ─────────────────────────────────────────────────────────────────

/// Provider-independent compaction strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStrategy {
    /// Keep the first `keep_initial` and last `keep_recent` turns; drop the rest.
    #[default]
    SlidingWindow,
    /// Score every middle turn by heuristic importance; drop the lowest-scoring ones.
    Importance,
}

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for [`CompactionEngine`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Compaction strategy.
    #[serde(default)]
    pub strategy: CompactionStrategy,

    /// Number of non-system messages that triggers compaction.
    #[serde(default = "CompactionConfig::default_max_messages")]
    pub max_messages: usize,

    /// Initial turns (from the start) to always keep.
    #[serde(default = "CompactionConfig::default_keep_initial")]
    pub keep_initial: usize,

    /// Most-recent turns to always keep.
    #[serde(default = "CompactionConfig::default_keep_recent")]
    pub keep_recent: usize,

    /// Minimum heuristic importance score to retain a middle turn
    /// (only used by the `Importance` strategy; range 0.0 – 1.0).
    #[serde(default = "CompactionConfig::default_importance_threshold")]
    pub importance_threshold: f64,
}

impl CompactionConfig {
    fn default_max_messages() -> usize {
        100
    }

    fn default_keep_initial() -> usize {
        5
    }

    fn default_keep_recent() -> usize {
        20
    }

    fn default_importance_threshold() -> f64 {
        0.5
    }
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            strategy: CompactionStrategy::SlidingWindow,
            max_messages: Self::default_max_messages(),
            keep_initial: Self::default_keep_initial(),
            keep_recent: Self::default_keep_recent(),
            importance_threshold: Self::default_importance_threshold(),
        }
    }
}

// ── Result ────────────────────────────────────────────────────────────────────

/// Statistics for a single compaction run.
#[derive(Debug)]
pub struct CompactionResult {
    /// Message count before compaction.
    pub before: usize,
    /// Message count after compaction.
    pub after: usize,
    /// Whether compaction actually reduced the history.
    pub compacted: bool,
    /// Strategy that was applied.
    pub strategy: CompactionStrategy,
}

impl CompactionResult {
    /// Fraction of messages removed (0.0 = nothing removed, 1.0 = everything removed).
    pub fn compression_ratio(&self) -> f64 {
        if self.before == 0 {
            return 0.0;
        }
        1.0 - (self.after as f64 / self.before as f64)
    }
}

// ── Engine ────────────────────────────────────────────────────────────────────

/// Provider-independent context compaction engine.
///
/// Complements `loop_::auto_compact_history` (which uses LLM summarisation)
/// with strategies that work offline or when no provider is available.
pub struct CompactionEngine {
    config: CompactionConfig,
}

impl CompactionEngine {
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// Returns `true` when the given message count exceeds the configured threshold.
    pub fn should_compact(&self, message_count: usize) -> bool {
        message_count > self.config.max_messages
    }

    /// Compact `messages` with the configured strategy.
    ///
    /// If compaction is not needed, returns the input unchanged.
    pub fn compact(
        &self,
        messages: Vec<ChatMessage>,
    ) -> (Vec<ChatMessage>, CompactionResult) {
        let before = messages.len();

        if !self.should_compact(before) {
            return (
                messages,
                CompactionResult {
                    before,
                    after: before,
                    compacted: false,
                    strategy: self.config.strategy,
                },
            );
        }

        let compacted = match self.config.strategy {
            CompactionStrategy::SlidingWindow => self.sliding_window(messages),
            CompactionStrategy::Importance => self.importance(messages),
        };

        let after = compacted.len();
        (
            compacted,
            CompactionResult {
                before,
                after,
                compacted: true,
                strategy: self.config.strategy,
            },
        )
    }

    // ── Strategies ────────────────────────────────────────────────────────────

    /// Keep first `keep_initial` + last `keep_recent`; insert a tombstone between them.
    fn sliding_window(&self, mut messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
        let total = messages.len();
        let keep_initial = self.config.keep_initial.min(total);
        let keep_recent = self.config.keep_recent.min(total);

        if keep_initial + keep_recent >= total {
            return messages;
        }

        let mut out: Vec<ChatMessage> = messages.drain(..keep_initial).collect();

        let removed = total - keep_initial - keep_recent;
        out.push(ChatMessage::system(format!(
            "[Context compacted: {removed} messages removed (sliding-window strategy)]"
        )));

        let recent_start = messages.len() - keep_recent;
        out.extend(messages.drain(recent_start..));
        out
    }

    /// Score every middle turn by heuristic importance; keep only the highest-scoring ones.
    fn importance(&self, messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
        let total = messages.len();
        let keep_initial = self.config.keep_initial.min(total);
        let keep_recent = self.config.keep_recent.min(total.saturating_sub(keep_initial));
        let recent_start = total.saturating_sub(keep_recent);
        // Target: keep at most ~67 % of max_messages from the middle.
        let target = self.config.max_messages.max(1) * 2 / 3;

        let mut keep: HashSet<usize> = (0..keep_initial).chain(recent_start..total).collect();

        // Score middle turns and admit the highest-scoring ones within budget.
        let mut middle: Vec<(usize, f64)> = (keep_initial..recent_start)
            .map(|i| (i, Self::heuristic_importance(&messages[i])))
            .collect();
        middle.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let budget = target.saturating_sub(keep.len());
        for (idx, score) in middle.into_iter().take(budget) {
            if score >= self.config.importance_threshold {
                keep.insert(idx);
            }
        }

        let kept = keep.len();
        let removed = total.saturating_sub(kept);
        let mut out = Vec::with_capacity(kept + usize::from(removed > 0));
        let mut tombstone_inserted = false;

        for (i, msg) in messages.into_iter().enumerate() {
            if keep.contains(&i) {
                out.push(msg);
            } else if !tombstone_inserted {
                out.push(ChatMessage::system(format!(
                    "[Context compacted: {removed} low-importance messages removed]"
                )));
                tombstone_inserted = true;
            }
        }

        out
    }

    // ── Heuristic importance scoring ──────────────────────────────────────────

    /// Assign an importance score (0.0 – 1.0) to a message without LLM inference.
    ///
    /// Scoring weights:
    /// - Role: system > user > assistant > other
    /// - Length: up to +0.3 for messages ≥ 1 000 chars
    /// - Keywords: error / warning / critical / decide / approve / todo → +0.15
    pub fn heuristic_importance(msg: &ChatMessage) -> f64 {
        let mut score: f64 = match msg.role.as_str() {
            "system" => 0.5,
            "user" => 0.4,
            "assistant" => 0.3,
            _ => 0.2,
        };

        // Length factor (0 – 0.3).
        score += (msg.content.len() as f64 / 1_000.0).min(1.0) * 0.3;

        // Keyword bonus.
        const KEYWORDS: &[&str] = &[
            "error", "warning", "critical", "fix", "bug", "todo", "action",
            "decide", "confirm", "approve", "important",
        ];
        let lower = msg.content.to_lowercase();
        if KEYWORDS.iter().any(|kw| lower.contains(kw)) {
            score += 0.15;
        }

        score.clamp(0.0, 1.0)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn msgs(n: usize) -> Vec<ChatMessage> {
        (0..n)
            .map(|i| ChatMessage::user(format!("Message {i}")))
            .collect()
    }

    fn engine(strategy: CompactionStrategy, max: usize) -> CompactionEngine {
        CompactionEngine::new(CompactionConfig {
            strategy,
            max_messages: max,
            keep_initial: 2,
            keep_recent: 3,
            importance_threshold: 0.3,
        })
    }

    // ── should_compact ────────────────────────────────────────────────────────

    #[test]
    fn should_compact_only_above_threshold() {
        let e = engine(CompactionStrategy::SlidingWindow, 10);
        assert!(!e.should_compact(10));
        assert!(e.should_compact(11));
    }

    // ── sliding window ────────────────────────────────────────────────────────

    #[test]
    fn sliding_window_structure() {
        let e = engine(CompactionStrategy::SlidingWindow, 10);
        let (out, result) = e.compact(msgs(20));

        assert!(result.compacted);
        assert_eq!(result.before, 20);
        // 2 initial + 1 tombstone + 3 recent = 6
        assert_eq!(out.len(), 6);
        assert_eq!(out[0].content, "Message 0");
        assert_eq!(out[1].content, "Message 1");
        assert!(out[2].content.contains("Context compacted"));
        assert_eq!(out[3].content, "Message 17");
        assert_eq!(out[4].content, "Message 18");
        assert_eq!(out[5].content, "Message 19");
    }

    #[test]
    fn sliding_window_no_compaction_below_threshold() {
        let e = engine(CompactionStrategy::SlidingWindow, 50);
        let (out, result) = e.compact(msgs(20));
        assert!(!result.compacted);
        assert_eq!(out.len(), 20);
    }

    #[test]
    fn sliding_window_compression_ratio() {
        let e = engine(CompactionStrategy::SlidingWindow, 10);
        let (_, result) = e.compact(msgs(20));
        assert!(result.compression_ratio() > 0.0);
        assert!(result.compression_ratio() < 1.0);
    }

    // ── importance ────────────────────────────────────────────────────────────

    #[test]
    fn importance_reduces_history() {
        let e = engine(CompactionStrategy::Importance, 10);
        let (out, result) = e.compact(msgs(20));
        assert!(result.compacted);
        assert!(out.len() < 20);
    }

    #[test]
    fn importance_keeps_initial_and_recent() {
        let e = engine(CompactionStrategy::Importance, 10);
        let input = msgs(20);
        let (out, _) = e.compact(input.clone());
        // First two messages must be preserved.
        assert_eq!(out[0].content, input[0].content);
        assert_eq!(out[1].content, input[1].content);
        // Last three messages must be preserved.
        let last3: Vec<_> = out.iter().rev().take(3).collect();
        assert!(last3
            .iter()
            .any(|m| m.content == input[19].content));
    }

    // ── heuristic importance ──────────────────────────────────────────────────

    #[test]
    fn error_keyword_raises_score() {
        let msg = ChatMessage::user("There's an error in the code");
        assert!(CompactionEngine::heuristic_importance(&msg) > 0.5);
    }

    #[test]
    fn short_acknowledgment_has_low_score() {
        let msg = ChatMessage::assistant("OK");
        assert!(CompactionEngine::heuristic_importance(&msg) < 0.5);
    }

    #[test]
    fn system_message_scores_high() {
        let msg = ChatMessage::system("System initialized");
        assert!(CompactionEngine::heuristic_importance(&msg) >= 0.5);
    }

    #[test]
    fn long_message_scores_high() {
        let msg = ChatMessage::user("x".repeat(1_000));
        assert!(CompactionEngine::heuristic_importance(&msg) > 0.6);
    }

    // ── compression ratio ─────────────────────────────────────────────────────

    #[test]
    fn compression_ratio_zero_before() {
        let r = CompactionResult {
            before: 0,
            after: 0,
            compacted: false,
            strategy: CompactionStrategy::SlidingWindow,
        };
        assert_eq!(r.compression_ratio(), 0.0);
    }

    #[test]
    fn compression_ratio_75_percent() {
        let r = CompactionResult {
            before: 100,
            after: 25,
            compacted: true,
            strategy: CompactionStrategy::SlidingWindow,
        };
        assert_eq!(r.compression_ratio(), 0.75);
    }
}
