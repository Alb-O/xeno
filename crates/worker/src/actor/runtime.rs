//! Actor lifecycle runtime orchestration entry points.

use super::handle::ActorHandle;
use super::spec::{Actor, ActorSpec};

/// Actor runtime orchestration entrypoint.
#[derive(Debug, Default, Clone, Copy)]
pub struct ActorRuntime;

impl ActorRuntime {
	/// Spawns one supervised actor from an [`ActorSpec`].
	pub fn spawn<A>(spec: ActorSpec<A>) -> ActorHandle<A::Cmd, A::Evt>
	where
		A: Actor,
	{
		crate::supervisor::spawn_supervised_actor(spec)
	}
}
