//! Actor spec and lifecycle type surface.

pub use crate::supervisor::{
	ActorContext, ActorExit, ActorExitKind, ActorFlow, ActorSpec, RestartPolicy as ActorRestartPolicy,
	ShutdownMode as ActorShutdownMode, ShutdownReport as ActorShutdownReport, SupervisorSpec as ActorLifecyclePolicy,
	WorkerActor as Actor,
};

/// Mailbox configuration for actor specs.
pub type ActorMailboxSpec = crate::supervisor::MailboxSpec;
