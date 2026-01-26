//! Unified async work scheduler with backpressure and cancellation.

mod ops;
mod state;
mod types;

#[cfg(test)]
mod tests;

pub use state::WorkScheduler;
pub use types::{DocId, WorkItem, WorkKind};
