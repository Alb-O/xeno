//! Shared worker runtime primitives.
//!
//! This crate centralizes task classification, spawn helpers, and join-set
//! orchestration used across core runtime subsystems.
//!
//! The API surface includes:
//! * task classification and runtime-scoped spawn helpers
//! * bounded mailbox policies
//! * generation-scoped cancellation tokens
//! * supervised actors with restart policies
//! * a runtime facade with bounded managed-work draining
//! * registry snapshots for worker status reporting

mod budget;
mod class;
mod join_set;
mod mailbox;
mod registry;
mod runtime;
pub mod spawn;
mod supervisor;
mod token;

pub use budget::{DrainBudget, DrainReport};
pub use class::TaskClass;
pub use join_set::WorkerJoinSet;
pub use mailbox::{Mailbox, MailboxPolicy, MailboxReceiver, MailboxSendError, MailboxSendOutcome, MailboxSender};
pub use registry::{WorkerRecord, WorkerRegistry};
pub use runtime::WorkerRuntime;
pub use supervisor::{
	ActorContext, ActorExitReason, ActorFlow, ActorHandle, ActorSpec, MailboxSpec, RestartPolicy, ShutdownMode, ShutdownReport, SupervisorSpec, WorkerActor,
	spawn_supervised_actor,
};
pub use token::{GenerationClock, GenerationToken};

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod panic_tests;

/// Extracts the panic payload message from a [`tokio::task::JoinError`].
///
/// Returns `None` if the error is a cancellation rather than a panic.
/// Handles both `String` and `&'static str` payloads; falls back to a
/// placeholder for other types.
pub fn join_error_panic_message(err: tokio::task::JoinError) -> Option<String> {
	if !err.is_panic() {
		return None;
	}
	let payload = err.into_panic();
	let msg = match payload.downcast::<String>() {
		Ok(s) => *s,
		Err(payload) => match payload.downcast::<&'static str>() {
			Ok(s) => (*s).to_string(),
			Err(_) => "<non-string panic payload>".to_string(),
		},
	};
	Some(msg)
}
