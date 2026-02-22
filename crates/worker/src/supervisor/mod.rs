use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{Mutex, broadcast};
use tokio_util::sync::CancellationToken;

use crate::TaskClass;
use crate::mailbox::{Mailbox, MailboxReceiver, MailboxSender};
use crate::token::{GenerationClock, GenerationToken};

mod join_ctrl;

use join_ctrl::SupervisorJoinCtrl;

/// Continuation directive from one actor command handling step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorFlow {
	/// Continue processing commands.
	Continue,
	/// Stop this actor instance.
	Stop,
}

/// Opaque exit classification for public consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActorExitKind {
	Stopped,
	MailboxClosed,
	Cancelled,
	StartupFailed,
	HandlerFailed,
	Panicked,
	JoinFailed,
}

/// Opaque exit summary for public consumers.
///
/// Wraps the exit classification and optional error message without
/// exposing the internal `ActorExitReason` enum variants that carry
/// payload strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorExit {
	kind: ActorExitKind,
	message: Option<String>,
}

impl ActorExit {
	pub fn kind(&self) -> ActorExitKind {
		self.kind
	}

	pub fn message(&self) -> Option<&str> {
		self.message.as_deref()
	}

	pub fn is_failure(&self) -> bool {
		matches!(
			self.kind,
			ActorExitKind::StartupFailed | ActorExitKind::HandlerFailed | ActorExitKind::Panicked | ActorExitKind::JoinFailed
		)
	}
}

impl From<&ActorExitReason> for ActorExit {
	fn from(reason: &ActorExitReason) -> Self {
		match reason {
			ActorExitReason::Stopped => Self {
				kind: ActorExitKind::Stopped,
				message: None,
			},
			ActorExitReason::MailboxClosed => Self {
				kind: ActorExitKind::MailboxClosed,
				message: None,
			},
			ActorExitReason::Cancelled => Self {
				kind: ActorExitKind::Cancelled,
				message: None,
			},
			ActorExitReason::StartupFailed(msg) => Self {
				kind: ActorExitKind::StartupFailed,
				message: Some(msg.clone()),
			},
			ActorExitReason::HandlerFailed(msg) => Self {
				kind: ActorExitKind::HandlerFailed,
				message: Some(msg.clone()),
			},
			ActorExitReason::Panicked => Self {
				kind: ActorExitKind::Panicked,
				message: None,
			},
			ActorExitReason::JoinFailed(msg) => Self {
				kind: ActorExitKind::JoinFailed,
				message: Some(msg.clone()),
			},
		}
	}
}

/// Exit reason for one supervised actor instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActorExitReason {
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
pub enum ActorRestartPolicy {
	Never,
	OnFailure { max_restarts: usize, backoff: Duration },
}

impl ActorRestartPolicy {
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
		}
	}
}

/// Mailbox sizing configuration for supervised actors.
///
/// The mailbox mode (backpressure vs coalesce) is determined by whether
/// `coalesce_by_key` is called on the `ActorSpec` builder.
#[derive(Debug, Clone)]
pub struct ActorMailboxSpec {
	pub(crate) capacity: usize,
}

impl ActorMailboxSpec {
	/// Creates a mailbox spec with the given capacity.
	///
	/// # Panics
	///
	/// Panics if `capacity` is zero.
	#[must_use]
	pub fn with_capacity(capacity: usize) -> Self {
		assert!(capacity > 0, "mailbox capacity must be > 0");
		Self { capacity }
	}
}

impl Default for ActorMailboxSpec {
	fn default() -> Self {
		Self { capacity: 128 }
	}
}

/// Supervisor configuration for one actor.
#[derive(Debug, Clone)]
pub struct ActorSupervisorSpec {
	pub(crate) restart: ActorRestartPolicy,
	pub(crate) event_buffer: usize,
}

impl ActorSupervisorSpec {
	/// Sets the restart policy.
	#[must_use]
	pub fn restart(mut self, restart: ActorRestartPolicy) -> Self {
		self.restart = restart;
		self
	}

	/// Sets the event broadcast buffer capacity.
	///
	/// # Panics
	///
	/// Panics if `size` is zero.
	#[must_use]
	pub fn event_buffer(mut self, size: usize) -> Self {
		assert!(size > 0, "event buffer size must be > 0");
		self.event_buffer = size;
		self
	}
}

impl Default for ActorSupervisorSpec {
	fn default() -> Self {
		Self {
			restart: ActorRestartPolicy::OnFailure {
				max_restarts: 3,
				backoff: Duration::from_millis(50),
			},
			event_buffer: 128,
		}
	}
}

/// Shutdown mode for supervised actors.
#[derive(Debug, Clone, Copy)]
pub enum ActorShutdownMode {
	Immediate,
	Graceful { timeout: Duration },
}

/// Shutdown report for one actor.
#[derive(Debug, Clone)]
pub struct ActorShutdownReport {
	completed: bool,
	timed_out: bool,
	last_exit: Option<ActorExit>,
}

impl ActorShutdownReport {
	pub fn completed(&self) -> bool {
		self.completed
	}

	pub fn timed_out(&self) -> bool {
		self.timed_out
	}

	pub fn last_exit(&self) -> Option<&ActorExit> {
		self.last_exit.as_ref()
	}
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
	pub(crate) name: String,
	pub(crate) class: TaskClass,
	pub(crate) mailbox: ActorMailboxSpec,
	pub(crate) supervisor: ActorSupervisorSpec,
	factory: Arc<dyn Fn() -> A + Send + Sync>,
	coalesce_eq: Option<Arc<CoalesceEqFn<A::Cmd>>>,
}

type CoalesceEqFn<T> = dyn Fn(&T, &T) -> bool + Send + Sync;

impl<A> ActorSpec<A>
where
	A: WorkerActor,
{
	/// Creates a new actor spec from a factory closure.
	pub fn new(name: impl Into<String>, class: TaskClass, factory: impl Fn() -> A + Send + Sync + 'static) -> Self {
		Self {
			name: name.into(),
			class,
			mailbox: ActorMailboxSpec::default(),
			supervisor: ActorSupervisorSpec::default(),
			factory: Arc::new(factory),
			coalesce_eq: None,
		}
	}

	/// Configures mailbox policy/capacity.
	#[must_use]
	pub fn mailbox(mut self, mailbox: ActorMailboxSpec) -> Self {
		self.mailbox = mailbox;
		self
	}

	/// Configures supervisor behavior.
	#[must_use]
	pub fn supervisor(mut self, supervisor: ActorSupervisorSpec) -> Self {
		self.supervisor = supervisor;
		self
	}

	/// Enables keyed coalescing mailboxes.
	#[must_use]
	pub fn coalesce_by_key<K>(mut self, key_fn: impl Fn(&A::Cmd) -> K + Send + Sync + 'static) -> Self
	where
		K: Eq + Send + Sync + 'static,
	{
		self.coalesce_eq = Some(Arc::new(move |lhs: &A::Cmd, rhs: &A::Cmd| key_fn(lhs) == key_fn(rhs)));
		self
	}
}

struct ActorState {
	generation: AtomicU64,
	restarts: AtomicUsize,
	last_exit: Mutex<Option<ActorExitReason>>,
}

/// Error returned when sending a command to a stopped actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorSendError {
	/// The actor's mailbox is closed.
	Closed,
}

impl From<crate::mailbox::MailboxSendError> for ActorSendError {
	fn from(_: crate::mailbox::MailboxSendError) -> Self {
		ActorSendError::Closed
	}
}

impl std::fmt::Display for ActorSendError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ActorSendError::Closed => write!(f, "actor mailbox closed"),
		}
	}
}

impl std::error::Error for ActorSendError {}

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
	join_ctrl: Arc<SupervisorJoinCtrl>,
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
	pub fn subscribe(&self) -> crate::ActorEventReceiver<Evt> {
		self.events.subscribe()
	}

	/// Sends one command honoring mailbox policy.
	pub async fn send(&self, cmd: Cmd) -> Result<(), ActorSendError> {
		self.tx.send(cmd).await?;
		Ok(())
	}

	/// Requests cancellation and closes the mailbox.
	///
	/// After cancel, the supervisor loop will exit and no restarts occur.
	/// Closing the mailbox eagerly ensures that subsequent `send()` calls
	/// fail fast instead of blocking on backpressure into a dead actor.
	pub fn cancel(&self) {
		self.cancel.cancel();
		self.tx.close_now();
	}

	/// Returns last exit summary observed by supervisor.
	pub async fn last_exit(&self) -> Option<ActorExit> {
		self.state.last_exit.lock().await.as_ref().map(ActorExit::from)
	}

	/// Shuts down this actor.
	pub async fn shutdown(&self, mode: ActorShutdownMode) -> ActorShutdownReport {
		match mode {
			ActorShutdownMode::Immediate => {
				self.cancel.cancel();
				self.tx.close().await;
				self.join_ctrl.join_forever().await;
				ActorShutdownReport {
					completed: true,
					timed_out: false,
					last_exit: self.last_exit().await,
				}
			}
			ActorShutdownMode::Graceful { timeout } => {
				self.tx.close().await;
				let completed = self.join_ctrl.join_with_timeout(timeout).await;
				if !completed {
					self.cancel.cancel();
				}
				ActorShutdownReport {
					completed,
					timed_out: !completed,
					last_exit: self.last_exit().await,
				}
			}
		}
	}

	/// Two-phase shutdown: tries graceful first, forces immediate on timeout.
	pub async fn shutdown_graceful_or_force(&self, timeout: Duration) -> ActorShutdownReport {
		let report = self.shutdown(ActorShutdownMode::Graceful { timeout }).await;
		if report.timed_out() {
			tracing::warn!(actor = %self.name, "graceful shutdown timed out; forcing immediate");
			return self.shutdown(ActorShutdownMode::Immediate).await;
		}
		report
	}
}

/// Spawns a supervised actor.
pub fn spawn_supervised_actor<A>(spec: ActorSpec<A>) -> ActorHandle<A::Cmd, A::Evt>
where
	A: WorkerActor,
{
	let mailbox = match spec.coalesce_eq {
		Some(eq_fn) => Mailbox::coalesce_by_eq(spec.mailbox.capacity, move |lhs: &A::Cmd, rhs: &A::Cmd| eq_fn(lhs, rhs)),
		None => Mailbox::backpressure(spec.mailbox.capacity),
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

	let supervisor_task = crate::spawn(task_class, async move {
		let mut restart_count = 0usize;
		// Per-generation cancel token, cancelled before each restart to kill
		// any lingering child tasks from the previous generation.
		let mut gen_cancel = CancellationToken::new();
		loop {
			if task_cancel.is_cancelled() {
				gen_cancel.cancel();
				let mut last = task_state.last_exit.lock().await;
				*last = Some(ActorExitReason::Cancelled);
				break;
			}

			// Cancel the previous generation's token to kill zombie child tasks.
			gen_cancel.cancel();
			gen_cancel = task_cancel.child_token();

			let gen_id = generation.next();
			task_state.generation.store(gen_id, Ordering::Release);
			let token = GenerationToken::new(gen_id, gen_cancel.child_token());
			let actor = (task_factory)();
			let child_rx = rx.clone();
			let child_events = task_events.clone();

			let child = crate::spawn(task_class, run_actor_instance(actor, child_rx, child_events, token));
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
		join_ctrl: Arc::new(SupervisorJoinCtrl::new(supervisor_task)),
	}
}

async fn run_actor_instance<A>(mut actor: A, rx: MailboxReceiver<A::Cmd>, events: broadcast::Sender<A::Evt>, token: GenerationToken) -> ActorExitReason
where
	A: WorkerActor,
{
	let mut ctx = ActorContext::new(events, token.clone());

	// Cancel-aware startup: preempt on_start if token fires mid-await.
	let started = tokio::select! {
		biased;
		_ = token.cancelled() => false,
		res = actor.on_start(&mut ctx) => {
			match res {
				Ok(()) => true,
				Err(err) => return ActorExitReason::StartupFailed(err),
			}
		}
	};
	if !started {
		return ActorExitReason::Cancelled;
	}

	let reason = loop {
		// Cancel-aware recv: preempt if token fires while waiting for mail.
		let cmd = tokio::select! {
			biased;
			_ = token.cancelled() => break ActorExitReason::Cancelled,
			msg = rx.recv() => {
				let Some(cmd) = msg else {
					break ActorExitReason::MailboxClosed;
				};
				cmd
			}
		};

		// Cancel-aware handle: preempt long-running handlers.
		let flow = tokio::select! {
			biased;
			_ = token.cancelled() => break ActorExitReason::Cancelled,
			res = actor.handle(cmd, &mut ctx) => res,
		};

		match flow {
			Ok(ActorFlow::Continue) => {}
			Ok(ActorFlow::Stop) => break ActorExitReason::Stopped,
			Err(err) => break ActorExitReason::HandlerFailed(err),
		}
	};

	actor.on_stop(&mut ctx).await;
	reason
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests;
