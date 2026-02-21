//! Actor framework surface built on worker runtime primitives.
//!
//! This namespace provides an actor-first API for specs, lifecycle policies,
//! mailbox behavior, command ingress, and control handles.

pub mod dispatch;
pub mod handle;
pub mod mailbox;
pub mod runtime;
pub mod spec;

pub use dispatch::{ActorCommandIngress, ActorCommandPort};
pub use handle::ActorHandle;
pub use mailbox::ActorMailboxSpec;
pub use runtime::ActorRuntime;
pub use spec::{Actor, ActorContext, ActorExit, ActorExitKind, ActorFlow, ActorLifecyclePolicy, ActorRestartPolicy, ActorShutdownMode, ActorShutdownReport, ActorSpec};
