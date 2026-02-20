//! Shared actor command ingress helpers.
//!
//! This avoids per-subsystem reimplementation of:
//! * `mpsc` command queue setup
//! * forwarding task lifecycle
//! * coordinated shutdown with actor teardown

use std::sync::Arc;

use tokio::sync::{Mutex, Notify, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::handle::ActorHandle;
use super::spec::{ActorShutdownMode, ActorShutdownReport};
use crate::TaskClass;
use crate::runtime::WorkerRuntime;

/// Cloneable actor command enqueue port.
pub struct ActorCommandPort<Cmd>
where
	Cmd: Send + 'static,
{
	tx: mpsc::UnboundedSender<Cmd>,
}

impl<Cmd> Clone for ActorCommandPort<Cmd>
where
	Cmd: Send + 'static,
{
	fn clone(&self) -> Self {
		Self { tx: self.tx.clone() }
	}
}

impl<Cmd> ActorCommandPort<Cmd>
where
	Cmd: Send + 'static,
{
	/// Enqueues one command for the actor ingress queue.
	pub fn send(&self, cmd: Cmd) -> Result<(), mpsc::error::SendError<Cmd>> {
		self.tx.send(cmd)
	}
}

enum JoinState {
	Handle(JoinHandle<()>),
	Joining,
	Done,
}

struct JoinCtrl {
	state: Mutex<JoinState>,
	done: Notify,
}

impl JoinCtrl {
	fn new(handle: JoinHandle<()>) -> Self {
		Self {
			state: Mutex::new(JoinState::Handle(handle)),
			done: Notify::new(),
		}
	}

	async fn join_forever(&self) {
		loop {
			let maybe_handle = {
				let mut state = self.state.lock().await;
				match &*state {
					JoinState::Done => return,
					JoinState::Joining => {
						let notified = self.done.notified();
						drop(state);
						notified.await;
						continue;
					}
					JoinState::Handle(_) => {
						let JoinState::Handle(handle) = std::mem::replace(&mut *state, JoinState::Joining) else {
							unreachable!()
						};
						Some(handle)
					}
				}
			};
			if let Some(handle) = maybe_handle {
				let _ = handle.await;
				*self.state.lock().await = JoinState::Done;
				self.done.notify_waiters();
				return;
			}
		}
	}
}

/// Actor command ingress queue backed by a framework-owned forwarding task.
pub struct ActorCommandIngress<Cmd, Evt>
where
	Cmd: Send + 'static,
	Evt: Clone + Send + 'static,
{
	port: ActorCommandPort<Cmd>,
	cancel: CancellationToken,
	actor: Arc<ActorHandle<Cmd, Evt>>,
	join_ctrl: Arc<JoinCtrl>,
}

impl<Cmd, Evt> ActorCommandIngress<Cmd, Evt>
where
	Cmd: Send + 'static,
	Evt: Clone + Send + 'static,
{
	/// Creates one ingress queue and starts a forwarding task.
	pub fn new(runtime: &WorkerRuntime, class: TaskClass, actor: Arc<ActorHandle<Cmd, Evt>>) -> Self {
		let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
		let cancel = CancellationToken::new();
		let task_cancel = cancel.clone();
		let task_actor = Arc::clone(&actor);
		let task = runtime.spawn(class, async move {
			loop {
				let cmd = tokio::select! {
					biased;
					_ = task_cancel.cancelled() => break,
					maybe_cmd = rx.recv() => {
						let Some(cmd) = maybe_cmd else {
							break;
						};
						cmd
					}
				};
				let send_result = tokio::select! {
					biased;
					_ = task_cancel.cancelled() => break,
					result = task_actor.send(cmd) => result,
				};
				if send_result.is_err() {
					break;
				}
			}
		});

		Self {
			port: ActorCommandPort { tx },
			cancel,
			actor,
			join_ctrl: Arc::new(JoinCtrl::new(task)),
		}
	}

	/// Enqueues one command for forwarding to the actor.
	pub fn send(&self, cmd: Cmd) -> Result<(), mpsc::error::SendError<Cmd>> {
		self.port.send(cmd)
	}

	/// Returns a cloneable enqueue port for shared command producers.
	pub fn port(&self) -> ActorCommandPort<Cmd> {
		self.port.clone()
	}

	/// Returns the underlying actor handle.
	pub fn actor(&self) -> &Arc<ActorHandle<Cmd, Evt>> {
		&self.actor
	}

	/// Returns an actor event subscription receiver.
	pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Evt> {
		self.actor.subscribe()
	}

	/// Cancels forwarding and joins the forwarding task.
	pub async fn stop_forwarding(&self) {
		self.cancel.cancel();
		self.join_ctrl.join_forever().await;
	}

	/// Stops forwarding and then shuts down the actor.
	pub async fn shutdown(&self, mode: ActorShutdownMode) -> ActorShutdownReport {
		self.stop_forwarding().await;
		self.actor.shutdown(mode).await
	}
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
	use std::time::Duration;

	use async_trait::async_trait;

	use super::*;
	use crate::{Actor, ActorContext, ActorFlow, ActorLifecyclePolicy, ActorRestartPolicy, ActorRuntime, ActorSpec};

	struct EchoActor;

	#[async_trait]
	impl Actor for EchoActor {
		type Cmd = usize;
		type Evt = usize;

		async fn handle(&mut self, cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			ctx.emit(cmd);
			Ok(ActorFlow::Continue)
		}
	}

	#[tokio::test]
	async fn ingress_forwards_commands_to_actor() {
		let runtime = WorkerRuntime::new();
		let actor = Arc::new(ActorRuntime::spawn(
			ActorSpec::new("dispatch.echo", crate::TaskClass::Background, || EchoActor).supervisor(ActorLifecyclePolicy {
				restart: ActorRestartPolicy::Never,
				event_buffer: 8,
			}),
		));
		let ingress = ActorCommandIngress::new(&runtime, crate::TaskClass::Background, Arc::clone(&actor));
		let mut events = ingress.subscribe();

		let _ = ingress.send(7);
		let _ = ingress.send(9);

		assert_eq!(events.recv().await.ok(), Some(7));
		assert_eq!(events.recv().await.ok(), Some(9));

		let report = ingress
			.shutdown(ActorShutdownMode::Graceful {
				timeout: Duration::from_secs(1),
			})
			.await;
		assert!(report.completed);
	}
}
