//! Deferred invocation mailbox model used by runtime pump convergence.
//!
//! # Purpose
//!
//! * Defines the deferred invocation envelope executed by `pump`.
//! * Carries explicit execution policy and scope tags per queued item.
//! * Provides targeted clearing primitives for stop-propagation semantics.
//!
//! # Mental model
//!
//! * Producers enqueue `Invocation` values with metadata:
//!   * source (diagnostics)
//!   * execution policy (log-only command path vs enforcing Nu pipeline path)
//!   * scope tag (global or Nu stop scope)
//! * Runtime pump drains items in FIFO order under a bounded per-round cap.
//! * Stop propagation clears only entries in the matching Nu scope tag.
//!
//! # Invariants
//!
//! * FIFO order must be preserved across all producers.
//! * Execution policy must be attached at enqueue time.
//! * Scope-based clearing must not remove unrelated queued invocations.

pub use crate::types::{DeferredInvocationExecutionPolicy, DeferredInvocationScope, DeferredInvocationSource};
