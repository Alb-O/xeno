//! Nu invocation coordination.
//!
//! Owns runtime/executor lifecycle plus the state machine backing the editor
//! Nu hook pipeline.
//!
//! The pipeline tracks:
//! * hook queue state
//! * in-flight async evaluations
//! * per-runtime generation tokens for stale-result protection
//! * schedule token validation for deferred invocation enqueue

pub(crate) mod errors;
pub(crate) mod runner;
mod state;

pub use state::NuScheduleFiredMsg;
#[allow(unused_imports)]
pub(crate) use state::{HookEvalFailureTransition, HookPipelinePhase, InFlightNuHook, NuCoordinatorState, NuEvalToken, QueuedNuHook};
