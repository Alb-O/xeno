use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::TaskClass;
use crate::mailbox::{Mailbox, MailboxPolicy, MailboxReceiver, MailboxSendError, MailboxSendOutcome, MailboxSender};
use crate::token::{GenerationClock, GenerationToken};

/// Continuation directive from one actor command handling step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorFlow {
	/// Continue processing commands.
	Continue,
	/// Stop this actor instance.
	Stop,
}

/// Exit reason for one supervised actor instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorExitReason {
	Stopped,
	MailboxClosed,
	Cancelled,
	StartupFailed(String),
	HandlerFailed(String),
	Panicked,
	JoinFailed(String),
}

/// Supervisor restart policy.
#[derive(Debug, Clone)]
pub enum RestartPolicy {
	Never,
	OnFailure { max_restarts: usize, backoff: Duration },
	Always { max_restarts: Option<usize>, backoff: Duration },
}

impl RestartPolicy {
	fn restart_delay(&self, reason: &ActorExitReason, restart_count: usize) -> Option<Duration> {
		match self {
			Self::Never => None,
			Self::OnFailure { max_restarts, backoff } => {
				let is_failure = matches!(
					reason,
					ActorExitReason::StartupFailed(_) | ActorExitReason::HandlerFailed(_) | ActorExitReason::Panicked | ActorExitReason::JoinFailed(_)
				);
				if is_failure && restart_count < *max_restarts { Some(*backoff) } else { None }
			}
			Self::Always { max_restarts, backoff } => {
				if max_restarts.is_some_and(|max| restart_count >= max) {
					None
				} else {
					Some(*backoff)
				}
			}
		}
	}
}

/// Mailbox configuration for supervised actors.
#[derive(Debug, Clone)]
pub struct MailboxSpec {
	pub capacity: usize,
	pub policy: MailboxPolicy,
}

impl Default for MailboxSpec {
	fn default() -> Self {
		Self {
			capacity: 128,
			policy: MailboxPolicy::Backpressure,
		}
	}
}

/// Supervisor configuration for one actor.
#[derive(Debug, Clone)]
pub struct SupervisorSpec {
	pub restart: RestartPolicy,
	pub event_buffer: usize,
}

impl Default for SupervisorSpec {
	fn default() -> Self {
		Self {
			restart: RestartPolicy::OnFailure {
				max_restarts: 3,
				backoff: Duration::from_millis(50),
			},
			event_buffer: 128,
		}
	}
}

/// Shutdown mode for supervised actors.
#[derive(Debug, Clone, Copy)]
pub enum ShutdownMode {
	Immediate,
	Graceful { timeout: Duration },
}

/// Shutdown report for one actor.
#[derive(Debug, Clone)]
pub struct ShutdownReport {
	pub completed: bool,
	pub timed_out: bool,
	pub last_exit: Option<ActorExitReason>,
}

/// Actor trait executed by the supervisor.
#[async_trait]
pub trait WorkerActor: Send + 'static {
	type Cmd: Send + 'static;
	type Evt: Clone + Send + 'static;

	async fn on_start(&mut self, _ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
		Ok(())
	}

	async fn on_stop(&mut self, _ctx: &mut ActorContext<Self::Evt>) {}

	async fn handle(&mut self, cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String>;
}

/// Actor execution context, including generation token and event emitter.
pub struct ActorContext<Evt> {
	events: broadcast::Sender<Evt>,
	token: GenerationToken,
}

impl<Evt> ActorContext<Evt>
where
	Evt: Clone + Send + 'static,
{
	fn new(events: broadcast::Sender<Evt>, token: GenerationToken) -> Self {
		Self { events, token }
	}

	/// Emits one actor event to subscribers.
	pub fn emit(&self, evt: Evt) {
		let _ = self.events.send(evt);
	}

	/// Returns this actor generation.
	pub fn generation(&self) -> u64 {
		self.token.generation()
	}

	/// Returns whether the actor has been cancelled.
	pub fn is_cancelled(&self) -> bool {
		self.token.is_cancelled()
	}
}

/// Builder spec for one supervised actor.
pub struct ActorSpec<A>
where
	A: WorkerActor,
{
	pub name: String,
	pub class: TaskClass,
	pub mailbox: MailboxSpec,
	pub supervisor: SupervisorSpec,
	factory: Arc<dyn Fn() -> A + Send + Sync>,
	coalesce_eq: Option<Arc<dyn Fn(&A::Cmd, &A::Cmd) -> bool + Send + Sync>>,
}

impl<A> ActorSpec<A>
where
	A: WorkerActor,
{
	/// Creates a new actor spec from a factory closure.
	pub fn new(name: impl Into<String>, class: TaskClass, factory: impl Fn() -> A + Send + Sync + 'static) -> Self {
		Self {
			name: name.into(),
			class,
			mailbox: MailboxSpec::default(),
			supervisor: SupervisorSpec::default(),
			factory: Arc::new(factory),
			coalesce_eq: None,
		}
	}

	/// Configures mailbox policy/capacity.
	pub fn mailbox(mut self, mailbox: MailboxSpec) -> Self {
		self.mailbox = mailbox;
		self
	}

	/// Configures supervisor behavior.
	pub fn supervisor(mut self, supervisor: SupervisorSpec) -> Self {
		self.supervisor = supervisor;
		self
	}

	/// Enables keyed coalescing mailboxes.
	pub fn coalesce_by_key<K>(mut self, key_fn: impl Fn(&A::Cmd) -> K + Send + Sync + 'static) -> Self
	where
		K: Eq + Send + Sync + 'static,
	{
		self.coalesce_eq = Some(Arc::new(move |lhs: &A::Cmd, rhs: &A::Cmd| key_fn(lhs) == key_fn(rhs)));
		self.mailbox.policy = MailboxPolicy::CoalesceByKey;
		self
	}
}

struct ActorState {
	generation: AtomicU64,
	restarts: AtomicUsize,
	last_exit: Mutex<Option<ActorExitReason>>,
}

/// Handle for one supervised actor.
pub struct ActorHandle<Cmd, Evt>
where
	Cmd: Send + 'static,
	Evt: Clone + Send + 'static,
{
	name: String,
	class: TaskClass,
	tx: MailboxSender<Cmd>,
	events: broadcast::Sender<Evt>,
	cancel: CancellationToken,
	state: Arc<ActorState>,
	supervisor_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl<Cmd, Evt> Drop for ActorHandle<Cmd, Evt>
where
	Cmd: Send + 'static,
	Evt: Clone + Send + 'static,
{
	fn drop(&mut self) {
		self.cancel.cancel();
		self.tx.close_now();
	}
}

impl<Cmd, Evt> ActorHandle<Cmd, Evt>
where
	Cmd: Send + 'static,
	Evt: Clone + Send + 'static,
{
	/// Actor name.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Worker class.
	pub const fn class(&self) -> TaskClass {
		self.class
	}

	/// Current generation.
	pub fn generation(&self) -> u64 {
		self.state.generation.load(Ordering::Acquire)
	}

	/// Number of supervised restarts.
	pub fn restart_count(&self) -> usize {
		self.state.restarts.load(Ordering::Acquire)
	}

	/// Subscribes to actor events.
	pub fn subscribe(&self) -> broadcast::Receiver<Evt> {
		self.events.subscribe()
	}

	/// Sends one command honoring mailbox policy.
	pub async fn send(&self, cmd: Cmd) -> Result<MailboxSendOutcome, MailboxSendError> {
		self.tx.send(cmd).await
	}

	/// Sends one command without waiting for mailbox capacity.
	pub async fn try_send(&self, cmd: Cmd) -> Result<MailboxSendOutcome, MailboxSendError> {
		self.tx.try_send(cmd).await
	}

	/// Requests cancellation.
	pub fn cancel(&self) {
		self.cancel.cancel();
	}

	/// Returns last exit reason observed by supervisor.
	pub async fn last_exit(&self) -> Option<ActorExitReason> {
		self.state.last_exit.lock().await.clone()
	}

	/// Shuts down this actor.
	pub async fn shutdown(&self, mode: ShutdownMode) -> ShutdownReport {
		match mode {
			ShutdownMode::Immediate => {
				self.cancel.cancel();
				self.tx.close().await;
				let mut task = self.supervisor_task.lock().await;
				if let Some(handle) = task.take() {
					let _ = handle.await;
				}
				ShutdownReport {
					completed: true,
					timed_out: false,
					last_exit: self.last_exit().await,
				}
			}
			ShutdownMode::Graceful { timeout } => {
				self.tx.close().await;
				let maybe_handle = {
					let mut task = self.supervisor_task.lock().await;
					task.take()
				};
				let Some(handle) = maybe_handle else {
					return ShutdownReport {
						completed: true,
						timed_out: false,
						last_exit: self.last_exit().await,
					};
				};

				match tokio::time::timeout(timeout, handle).await {
					Ok(_) => ShutdownReport {
						completed: true,
						timed_out: false,
						last_exit: self.last_exit().await,
					},
					Err(_) => {
						self.cancel.cancel();
						ShutdownReport {
							completed: false,
							timed_out: true,
							last_exit: self.last_exit().await,
						}
					}
				}
			}
		}
	}
}

/// Spawns a supervised actor.
pub fn spawn_supervised_actor<A>(spec: ActorSpec<A>) -> ActorHandle<A::Cmd, A::Evt>
where
	A: WorkerActor,
{
	let mailbox = match spec.coalesce_eq {
		Some(eq_fn) => Mailbox::with_coalesce_eq(spec.mailbox.capacity, move |lhs: &A::Cmd, rhs: &A::Cmd| eq_fn(lhs, rhs)),
		None => Mailbox::new(spec.mailbox.capacity, spec.mailbox.policy),
	};
	let tx = mailbox.sender();
	let rx = mailbox.receiver();

	let (events, _) = broadcast::channel(spec.supervisor.event_buffer.max(1));
	let cancel = CancellationToken::new();
	let state = Arc::new(ActorState {
		generation: AtomicU64::new(0),
		restarts: AtomicUsize::new(0),
		last_exit: Mutex::new(None),
	});
	let task_state = Arc::clone(&state);
	let task_cancel = cancel.clone();
	let task_events = events.clone();
	let task_name = spec.name.clone();
	let task_class = spec.class;
	let task_factory = Arc::clone(&spec.factory);
	let task_restart = spec.supervisor.restart.clone();
	let generation = GenerationClock::new();

	let supervisor_task = tokio::spawn(async move {
		let mut restart_count = 0usize;
		loop {
			if task_cancel.is_cancelled() {
				let mut last = task_state.last_exit.lock().await;
				*last = Some(ActorExitReason::Cancelled);
				break;
			}

			let gen_id = generation.next();
			task_state.generation.store(gen_id, Ordering::Release);
			let token = GenerationToken::new(gen_id, task_cancel.child_token());
			let actor = (task_factory)();
			let child_rx = rx.clone();
			let child_events = task_events.clone();

			let child = tokio::spawn(run_actor_instance(actor, child_rx, child_events, token));
			let reason = match child.await {
				Ok(reason) => reason,
				Err(err) if err.is_panic() => ActorExitReason::Panicked,
				Err(err) if err.is_cancelled() => ActorExitReason::Cancelled,
				Err(err) => ActorExitReason::JoinFailed(err.to_string()),
			};

			{
				let mut last = task_state.last_exit.lock().await;
				*last = Some(reason.clone());
			}

			tracing::debug!(
				actor = %task_name,
				class = ?task_class,
				generation = gen_id,
				restarts = restart_count,
				reason = ?reason,
				"worker.actor.exit"
			);

			if task_cancel.is_cancelled() {
				break;
			}

			let Some(backoff) = task_restart.restart_delay(&reason, restart_count) else {
				break;
			};

			restart_count = restart_count.wrapping_add(1);
			task_state.restarts.store(restart_count, Ordering::Release);
			if backoff > Duration::ZERO {
				tokio::select! {
					_ = task_cancel.cancelled() => break,
					_ = tokio::time::sleep(backoff) => {}
				}
			}
		}
	});

	ActorHandle {
		name: spec.name,
		class: spec.class,
		tx,
		events,
		cancel,
		state,
		supervisor_task: Arc::new(Mutex::new(Some(supervisor_task))),
	}
}

async fn run_actor_instance<A>(mut actor: A, rx: MailboxReceiver<A::Cmd>, events: broadcast::Sender<A::Evt>, token: GenerationToken) -> ActorExitReason
where
	A: WorkerActor,
{
	let mut ctx = ActorContext::new(events, token.clone());
	if let Err(err) = actor.on_start(&mut ctx).await {
		return ActorExitReason::StartupFailed(err);
	}

	loop {
		tokio::select! {
			_ = token.cancelled() => {
				actor.on_stop(&mut ctx).await;
				return ActorExitReason::Cancelled;
			}
			msg = rx.recv() => {
				let Some(cmd) = msg else {
					actor.on_stop(&mut ctx).await;
					return ActorExitReason::MailboxClosed;
				};

				match actor.handle(cmd, &mut ctx).await {
					Ok(ActorFlow::Continue) => {}
					Ok(ActorFlow::Stop) => {
						actor.on_stop(&mut ctx).await;
						return ActorExitReason::Stopped;
					}
					Err(err) => {
						actor.on_stop(&mut ctx).await;
						return ActorExitReason::HandlerFailed(err);
					}
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	use super::*;

	#[derive(Default)]
	struct CountingActor {
		seen: usize,
	}

	#[async_trait]
	impl WorkerActor for CountingActor {
		type Cmd = usize;
		type Evt = usize;

		async fn handle(&mut self, cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			self.seen = self.seen.wrapping_add(1);
			ctx.emit(cmd);
			if cmd == 99 { Ok(ActorFlow::Stop) } else { Ok(ActorFlow::Continue) }
		}
	}

	#[tokio::test]
	async fn actor_emits_events_and_stops() {
		let handle = spawn_supervised_actor(ActorSpec::new("counting", TaskClass::Interactive, CountingActor::default));
		let mut events = handle.subscribe();
		let _ = handle.send(1).await;
		let _ = handle.send(99).await;

		assert_eq!(events.recv().await.ok(), Some(1));
		assert_eq!(events.recv().await.ok(), Some(99));

		let report = handle
			.shutdown(ShutdownMode::Graceful {
				timeout: Duration::from_secs(1),
			})
			.await;
		assert!(report.completed);
	}

	struct FailingActor {
		start_counter: Arc<AtomicUsize>,
	}

	#[async_trait]
	impl WorkerActor for FailingActor {
		type Cmd = ();
		type Evt = ();

		async fn on_start(&mut self, _ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
			self.start_counter.fetch_add(1, Ordering::SeqCst);
			Ok(())
		}

		async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			Err("boom".to_string())
		}
	}

	#[tokio::test]
	async fn supervisor_restarts_on_handler_failure() {
		let starts = Arc::new(AtomicUsize::new(0));
		let starts_clone = Arc::clone(&starts);
		let spec = ActorSpec::new("failing", TaskClass::Background, move || FailingActor {
			start_counter: Arc::clone(&starts_clone),
		})
		.supervisor(SupervisorSpec {
			restart: RestartPolicy::OnFailure {
				max_restarts: 2,
				backoff: Duration::from_millis(1),
			},
			event_buffer: 8,
		});

		let handle = spawn_supervised_actor(spec);
		let _ = handle.send(()).await;
		tokio::time::sleep(Duration::from_millis(20)).await;
		handle.cancel();
		let _ = handle.shutdown(ShutdownMode::Immediate).await;

		assert!(starts.load(Ordering::SeqCst) >= 2, "actor should restart after failure");
	}
}
