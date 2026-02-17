//! Shared worker runtime primitives.
//!
//! This crate centralizes task classification, spawn helpers, and join-set
//! orchestration used across core runtime subsystems.

mod class;
mod join_set;
mod spawn;

pub use class::TaskClass;
pub use join_set::WorkerJoinSet;
pub use spawn::{spawn, spawn_blocking, spawn_named_thread, spawn_thread};
