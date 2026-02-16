//! Nu invocation coordination.
//!
//! Owns runtime/executor lifecycle plus hook/macro bookkeeping state used by
//! editor invocation dispatch.

pub(crate) mod errors;
pub(crate) mod runner;
mod state;

pub(crate) use state::{InFlightNuHook, NuCoordinatorState, QueuedNuHook};
