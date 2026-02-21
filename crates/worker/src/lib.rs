//! Shared worker runtime primitives.
//!
//! This crate centralizes task classification, spawn helpers, and join-set
//! orchestration used across core runtime subsystems.
//!
//! The API surface includes:
//! * task classification and runtime-scoped spawn helpers
//! * actor mailbox policies
//! * generation-scoped cancellation tokens
//! * actor runtime lifecycle orchestration with restart policies
//! * a runtime facade for task submission and actor spawning
//! * opaque actor exit summaries for public consumers
//!
//! This crate is Tokio-based. Public handle types ([`TaskHandle`],
//! [`ThreadHandle`], [`ActorEventReceiver`]) are aliases for their
//! Tokio/std equivalents.

pub mod actor;
mod budget;
mod class;
mod join_set;
mod mailbox;
mod runtime;
pub mod spawn;
mod supervisor;
mod token;

pub use actor::{
	Actor, ActorCommandIngress, ActorCommandPort, ActorContext, ActorExit, ActorExitKind, ActorFlow, ActorHandle,
	ActorMailboxSpec, ActorRestartPolicy, ActorRuntime, ActorShutdownMode, ActorShutdownReport, ActorSpec,
	ActorSupervisorSpec,
};
pub use supervisor::ActorSendError;
pub use class::TaskClass;
pub use join_set::WorkerJoinSet;
pub use runtime::WorkerRuntime;

/// Handle for an async task spawned on the Tokio runtime.
pub type TaskHandle<T> = tokio::task::JoinHandle<T>;

/// Error from joining a [`TaskHandle`].
pub type TaskJoinError = tokio::task::JoinError;

/// Handle for a thread spawned via [`WorkerRuntime::spawn_thread`].
pub type ThreadHandle<T> = std::thread::JoinHandle<T>;

/// Receiver for actor events from [`ActorHandle::subscribe`].
pub type ActorEventReceiver<E> = tokio::sync::broadcast::Receiver<E>;

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod panic_tests;

/// Extracts the panic payload message from a [`TaskJoinError`].
///
/// Returns `None` if the error is a cancellation rather than a panic.
/// Handles both `String` and `&'static str` payloads; falls back to a
/// placeholder for other types.
pub fn join_error_panic_message(err: TaskJoinError) -> Option<String> {
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
