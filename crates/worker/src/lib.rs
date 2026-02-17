//! Shared worker runtime primitives.
//!
//! This crate centralizes task classification, spawn helpers, and join-set
//! orchestration used across core runtime subsystems.
//!
//! The API surface includes:
//! * task classification and spawn helpers
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
mod spawn;
mod supervisor;
mod token;

pub use budget::{DrainBudget, DrainReport};
pub use class::TaskClass;
pub use join_set::WorkerJoinSet;
pub use mailbox::{Mailbox, MailboxPolicy, MailboxReceiver, MailboxSendError, MailboxSendOutcome, MailboxSender};
pub use registry::{WorkerRecord, WorkerRegistry};
pub use runtime::WorkerRuntime;
pub use spawn::{spawn, spawn_blocking, spawn_named_thread, spawn_thread};
pub use supervisor::{
	ActorContext, ActorExitReason, ActorFlow, ActorHandle, ActorSpec, MailboxSpec, RestartPolicy, ShutdownMode, ShutdownReport, SupervisorSpec, WorkerActor,
	spawn_supervised_actor,
};
pub use token::{GenerationClock, GenerationToken};
