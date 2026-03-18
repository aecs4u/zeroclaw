#[allow(clippy::module_inception)]
pub mod agent;
pub mod classifier;
pub mod compaction;
pub mod dispatcher;
pub mod loop_;
pub mod loop_detector;
pub mod memory_loader;
pub mod prompt;
pub mod thinking;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use agent::{Agent, AgentBuilder, TurnEvent};
#[allow(unused_imports)]
pub use compaction::{CompactionConfig, CompactionEngine, CompactionResult, CompactionStrategy};
#[allow(unused_imports)]
pub use loop_::{process_message, run};
