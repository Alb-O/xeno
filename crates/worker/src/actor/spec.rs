//! Actor spec and lifecycle type surface.

pub use crate::supervisor::{
	ActorContext, ActorExit, ActorExitKind, ActorFlow, ActorMailboxSpec, ActorRestartPolicy, ActorShutdownMode, ActorShutdownReport, ActorSpec,
	ActorSupervisorSpec, WorkerActor as Actor,
};
