use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::mailbox::{Mailbox, MailboxReceiver, MailboxSender};
use crate::token::{GenerationClock, GenerationToken};
use crate::{TaskClass, spawn};

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
			ActorExitReason::Stopped => Self { kind: ActorExitKind::Stopped, message: None },
			ActorExitReason::MailboxClosed => Self { kind: ActorExitKind::MailboxClosed, message: None },
			ActorExitReason::Cancelled => Self { kind: ActorExitKind::Cancelled, message: None },
			ActorExitReason::StartupFailed(msg) => Self { kind: ActorExitKind::StartupFailed, message: Some(msg.clone()) },
			ActorExitReason::HandlerFailed(msg) => Self { kind: ActorExitKind::HandlerFailed, message: Some(msg.clone()) },
			ActorExitReason::Panicked => Self { kind: ActorExitKind::Panicked, message: None },
			ActorExitReason::JoinFailed(msg) => Self { kind: ActorExitKind::JoinFailed, message: Some(msg.clone()) },
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
pub enum RestartPolicy {
	Never,
	OnFailure { max_restarts: usize, backoff: Duration },
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
		}
	}
}

/// Mailbox sizing configuration for supervised actors.
///
/// The mailbox mode (backpressure vs coalesce) is determined by whether
/// `coalesce_by_key` is called on the `ActorSpec` builder.
#[derive(Debug, Clone)]
pub struct MailboxSpec {
	pub(crate) capacity: usize,
}

impl MailboxSpec {
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

impl Default for MailboxSpec {
	fn default() -> Self {
		Self { capacity: 128 }
	}
}

/// Supervisor configuration for one actor.
#[derive(Debug, Clone)]
pub struct SupervisorSpec {
	pub(crate) restart: RestartPolicy,
	pub(crate) event_buffer: usize,
}

impl SupervisorSpec {
	/// Sets the restart policy.
	#[must_use]
	pub fn restart(mut self, restart: RestartPolicy) -> Self {
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
	completed: bool,
	timed_out: bool,
	last_exit: Option<ActorExit>,
}

impl ShutdownReport {
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
	pub(crate) mailbox: MailboxSpec,
	pub(crate) supervisor: SupervisorSpec,
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
			mailbox: MailboxSpec::default(),
			supervisor: SupervisorSpec::default(),
			factory: Arc::new(factory),
			coalesce_eq: None,
		}
	}

	/// Configures mailbox policy/capacity.
	#[must_use]
	pub fn mailbox(mut self, mailbox: MailboxSpec) -> Self {
		self.mailbox = mailbox;
		self
	}

	/// Configures supervisor behavior.
	#[must_use]
	pub fn supervisor(mut self, supervisor: SupervisorSpec) -> Self {
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

/// Join coordination state machine for the supervisor task.
///
/// Prevents concurrent shutdown callers from racing: only one caller
/// becomes the "leader" that awaits the join handle, all others wait
/// on the notify until the leader transitions to `Done`.
enum JoinState {
	/// Supervisor task is still owned; first caller to `shutdown` takes it.
	Handle(JoinHandle<()>),
	/// A caller is currently awaiting the join handle.
	Joining,
	/// Supervisor task has completed.
	Done,
}

struct SupervisorJoinCtrl {
	state: Mutex<JoinState>,
	done: tokio::sync::Notify,
}

impl SupervisorJoinCtrl {
	fn new(handle: JoinHandle<()>) -> Self {
		Self {
			state: Mutex::new(JoinState::Handle(handle)),
			done: tokio::sync::Notify::new(),
		}
	}

	/// Joins the supervisor task, blocking until done. Multiple callers are safe.
	async fn join_forever(&self) {
		loop {
			let maybe_handle = {
				let mut st = self.state.lock().await;
				match &*st {
					JoinState::Done => return,
					JoinState::Joining => {
						// Create Notified while lock is held to avoid lost-wakeup race:
						// leader could notify_waiters() between our drop(st) and .await.
						let notified = self.done.notified();
						drop(st);
						notified.await;
						continue;
					}
					JoinState::Handle(_) => {
						// Take the handle — we're the leader.
						let JoinState::Handle(h) = std::mem::replace(&mut *st, JoinState::Joining) else {
							unreachable!()
						};
						Some(h)
					}
				}
			};
			if let Some(h) = maybe_handle {
				let _ = h.await;
				*self.state.lock().await = JoinState::Done;
				self.done.notify_waiters();
				return;
			}
		}
	}

	/// Joins with a deadline. Returns `true` if completed, `false` if timed out.
	async fn join_with_timeout(&self, timeout: Duration) -> bool {
		let deadline = tokio::time::Instant::now() + timeout;
		loop {
			let maybe_handle = {
				let mut st = self.state.lock().await;
				match &*st {
					JoinState::Done => return true,
					JoinState::Joining => {
						// Create Notified while lock is held to avoid lost-wakeup race.
						let notified = self.done.notified();
						drop(st);
						tokio::select! {
							_ = notified => continue,
							_ = tokio::time::sleep_until(deadline) => return false,
						}
					}
					JoinState::Handle(_) => {
						let JoinState::Handle(h) = std::mem::replace(&mut *st, JoinState::Joining) else {
							unreachable!()
						};
						Some(h)
					}
				}
			};
			if let Some(mut h) = maybe_handle {
				tokio::select! {
					res = &mut h => {
						let _ = res;
						*self.state.lock().await = JoinState::Done;
						self.done.notify_waiters();
						return true;
					}
					_ = tokio::time::sleep_until(deadline) => {
						// Put the handle back — we couldn't finish in time.
						*self.state.lock().await = JoinState::Handle(h);
						self.done.notify_waiters();
						return false;
					}
				}
			}
		}
	}
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
	pub fn subscribe(&self) -> broadcast::Receiver<Evt> {
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
	pub async fn shutdown(&self, mode: ShutdownMode) -> ShutdownReport {
		match mode {
			ShutdownMode::Immediate => {
				self.cancel.cancel();
				self.tx.close().await;
				self.join_ctrl.join_forever().await;
				ShutdownReport {
					completed: true,
					timed_out: false,
					last_exit: self.last_exit().await,
				}
			}
			ShutdownMode::Graceful { timeout } => {
				self.tx.close().await;
				let completed = self.join_ctrl.join_with_timeout(timeout).await;
				if !completed {
					self.cancel.cancel();
				}
				ShutdownReport {
					completed,
					timed_out: !completed,
					last_exit: self.last_exit().await,
				}
			}
		}
	}

	/// Two-phase shutdown: tries graceful first, forces immediate on timeout.
	pub async fn shutdown_graceful_or_force(&self, timeout: Duration) -> ShutdownReport {
		let report = self.shutdown(ShutdownMode::Graceful { timeout }).await;
		if report.timed_out() {
			tracing::warn!(actor = %self.name, "graceful shutdown timed out; forcing immediate");
			return self.shutdown(ShutdownMode::Immediate).await;
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

	let supervisor_task = spawn::spawn(task_class, async move {
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

			let child = spawn::spawn(task_class, run_actor_instance(actor, child_rx, child_events, token));
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
		assert!(report.completed());
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

	struct SlowActor;

	#[async_trait]
	impl WorkerActor for SlowActor {
		type Cmd = ();
		type Evt = &'static str;

		async fn handle(&mut self, _cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			ctx.emit("entered");
			tokio::time::sleep(Duration::from_secs(60)).await;
			Ok(ActorFlow::Continue)
		}
	}

	#[tokio::test]
	async fn immediate_shutdown_preempts_slow_handler() {
		let handle = spawn_supervised_actor(ActorSpec::new("slow", TaskClass::Background, || SlowActor).supervisor(SupervisorSpec {
			restart: RestartPolicy::Never,
			event_buffer: 8,
		}));
		let mut events = handle.subscribe();

		let _ = handle.send(()).await;
		// Wait until handle() is entered (event emitted before the long sleep).
		let got = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
		assert_eq!(got.ok().and_then(|r| r.ok()), Some("entered"), "actor should enter handle()");

		// Immediate shutdown must complete quickly despite the 60s sleep.
		let report = tokio::time::timeout(Duration::from_millis(500), handle.shutdown(ShutdownMode::Immediate))
			.await
			.expect("shutdown should not hang");
		assert!(report.completed());
		assert_eq!(report.last_exit().map(|e| e.kind()), Some(ActorExitKind::Cancelled));
	}

	struct SlowStopActor {
		stopped: Arc<std::sync::atomic::AtomicBool>,
	}

	#[async_trait]
	impl WorkerActor for SlowStopActor {
		type Cmd = ();
		type Evt = &'static str;

		async fn handle(&mut self, _cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			ctx.emit("entered");
			tokio::time::sleep(Duration::from_secs(60)).await;
			Ok(ActorFlow::Continue)
		}

		async fn on_stop(&mut self, _ctx: &mut ActorContext<Self::Evt>) {
			tokio::time::sleep(Duration::from_millis(200)).await;
			self.stopped.store(true, Ordering::SeqCst);
		}
	}

	#[tokio::test]
	async fn graceful_timeout_retains_handle_for_immediate_followup() {
		let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let stopped_clone = Arc::clone(&stopped);
		let handle = spawn_supervised_actor(
			ActorSpec::new("slow-stop", TaskClass::Background, move || SlowStopActor {
				stopped: Arc::clone(&stopped_clone),
			})
			.supervisor(SupervisorSpec {
				restart: RestartPolicy::Never,
				event_buffer: 8,
			}),
		);
		let mut events = handle.subscribe();

		let _ = handle.send(()).await;
		let got = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
		assert_eq!(got.ok().and_then(|r| r.ok()), Some("entered"));

		// Graceful with very short timeout — will time out while handle() sleeps.
		let report = handle
			.shutdown(ShutdownMode::Graceful {
				timeout: Duration::from_millis(10),
			})
			.await;
		assert!(report.timed_out());
		assert!(!report.completed());
		// on_stop hasn't run yet (cancel just fired, actor still tearing down).
		assert!(!stopped.load(Ordering::SeqCst));

		// Follow-up Immediate must join the supervisor and wait for on_stop to finish.
		let report = tokio::time::timeout(Duration::from_secs(2), handle.shutdown(ShutdownMode::Immediate))
			.await
			.expect("immediate after graceful should not hang");
		assert!(report.completed());
		assert!(stopped.load(Ordering::SeqCst), "on_stop should have completed");
	}

	#[derive(Default)]
	struct NoopActor;

	#[async_trait]
	impl WorkerActor for NoopActor {
		type Cmd = ();
		type Evt = ();

		async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			Ok(ActorFlow::Continue)
		}
	}

	#[tokio::test]
	async fn graceful_shutdown_terminates_with_restart_on_failure() {
		let handle = spawn_supervised_actor(
			ActorSpec::new("restart-shutdown", TaskClass::Background, NoopActor::default).supervisor(SupervisorSpec {
				restart: RestartPolicy::OnFailure {
					max_restarts: 5,
					backoff: Duration::from_millis(1),
				},
				event_buffer: 8,
			}),
		);

		// Don't send anything — just graceful-shutdown immediately.
		let report = handle
			.shutdown(ShutdownMode::Graceful {
				timeout: Duration::from_millis(200),
			})
			.await;
		assert!(report.completed(), "graceful shutdown should complete promptly");
		assert!(!report.timed_out());
	}

	#[tokio::test]
	async fn shutdown_graceful_or_force_completes_with_slow_stop() {
		let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let stopped_clone = Arc::clone(&stopped);
		let handle = spawn_supervised_actor(
			ActorSpec::new("force-test", TaskClass::Background, move || SlowStopActor {
				stopped: Arc::clone(&stopped_clone),
			})
			.supervisor(SupervisorSpec {
				restart: RestartPolicy::Never,
				event_buffer: 8,
			}),
		);
		let mut events = handle.subscribe();

		let _ = handle.send(()).await;
		let got = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
		assert_eq!(got.ok().and_then(|r| r.ok()), Some("entered"));

		// Graceful with tiny timeout will time out (handler sleeps 60s),
		// then force immediate which cancels the handler + runs on_stop.
		let report = tokio::time::timeout(Duration::from_secs(2), handle.shutdown_graceful_or_force(Duration::from_millis(10)))
			.await
			.expect("shutdown_graceful_or_force should not hang");
		assert!(report.completed());
		assert!(stopped.load(Ordering::SeqCst), "on_stop should have completed via forced immediate");
	}

	#[tokio::test]
	async fn cancel_closes_mailbox_and_send_fails_fast() {
		let handle = spawn_supervised_actor(
			ActorSpec::new("cancel-close", TaskClass::Background, CountingActor::default).mailbox(MailboxSpec { capacity: 1 }),
		);

		handle.cancel();

		// send() must fail fast (not block on backpressure) since mailbox is closed.
		let result = tokio::time::timeout(Duration::from_millis(50), handle.send(1)).await;
		assert!(result.is_ok(), "send should not block after cancel");
		assert!(result.unwrap().is_err(), "send should return error on closed mailbox");

		let report = handle.shutdown(ShutdownMode::Immediate).await;
		assert!(report.completed());
	}

	struct ConcurrentStopActor {
		started_stop: Arc<std::sync::atomic::AtomicBool>,
		done_stop: Arc<std::sync::atomic::AtomicBool>,
	}

	#[async_trait]
	impl WorkerActor for ConcurrentStopActor {
		type Cmd = ();
		type Evt = &'static str;

		async fn handle(&mut self, _cmd: Self::Cmd, ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			ctx.emit("entered");
			tokio::time::sleep(Duration::from_secs(60)).await;
			Ok(ActorFlow::Continue)
		}

		async fn on_stop(&mut self, _ctx: &mut ActorContext<Self::Evt>) {
			self.started_stop.store(true, Ordering::SeqCst);
			tokio::time::sleep(Duration::from_millis(200)).await;
			self.done_stop.store(true, Ordering::SeqCst);
		}
	}

	#[tokio::test]
	async fn concurrent_shutdown_waits_for_in_progress_join() {
		let started_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let done_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let ss = Arc::clone(&started_stop);
		let ds = Arc::clone(&done_stop);
		let handle = Arc::new(spawn_supervised_actor(
			ActorSpec::new("concurrent-shutdown", TaskClass::Background, move || ConcurrentStopActor {
				started_stop: Arc::clone(&ss),
				done_stop: Arc::clone(&ds),
			})
			.supervisor(SupervisorSpec {
				restart: RestartPolicy::Never,
				event_buffer: 8,
			}),
		));
		let mut events = handle.subscribe();

		let _ = handle.send(()).await;
		let got = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
		assert_eq!(got.ok().and_then(|r| r.ok()), Some("entered"));

		// Caller A starts Immediate shutdown.
		let handle_a = Arc::clone(&handle);
		let task_a = tokio::spawn(async move { handle_a.shutdown(ShutdownMode::Immediate).await });

		// Wait until on_stop has started (proves A is the leader, joining).
		while !started_stop.load(Ordering::SeqCst) {
			tokio::task::yield_now().await;
		}
		// on_stop started but not done yet.
		assert!(!done_stop.load(Ordering::SeqCst));

		// Caller B also calls Immediate shutdown concurrently.
		let report_b = tokio::time::timeout(Duration::from_secs(2), handle.shutdown(ShutdownMode::Immediate))
			.await
			.expect("concurrent shutdown B should not hang");
		// B must wait until on_stop finishes (not return early).
		assert!(report_b.completed);
		assert!(done_stop.load(Ordering::SeqCst), "concurrent caller must see on_stop completed");

		let report_a = task_a.await.unwrap();
		assert!(report_a.completed);
	}

	// ── Restart + cancellation invariant tests ──

	/// Actor that tracks generation token cancellation across restarts.
	///
	/// On start, spawns a background task scoped to the generation token.
	/// Each tick increments `active_tickers`. On cancel, decrements it.
	/// If the supervisor properly cancels old generations on restart,
	/// `active_tickers` should never exceed 1.
	struct ZombieDetectorActor {
		active_tickers: Arc<AtomicUsize>,
		peak_tickers: Arc<AtomicUsize>,
		starts: Arc<AtomicUsize>,
		fail_first_n: Arc<AtomicUsize>,
	}

	#[async_trait]
	impl WorkerActor for ZombieDetectorActor {
		type Cmd = ();
		type Evt = ();

		async fn on_start(&mut self, ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
			let generation_id = ctx.generation();
			let count = self.starts.fetch_add(1, Ordering::SeqCst) + 1;
			let remaining_failures = self.fail_first_n.load(Ordering::SeqCst);

			// Spawn a background "ticker" scoped to this generation's token.
			let active = Arc::clone(&self.active_tickers);
			let peak = Arc::clone(&self.peak_tickers);
			let token = ctx.token.child();
			tokio::spawn(async move {
				let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
				// Track peak concurrent tickers.
				peak.fetch_max(cur, Ordering::SeqCst);

				// Keep ticking until cancelled.
				loop {
					tokio::select! {
						biased;
						_ = token.cancelled() => break,
						_ = tokio::time::sleep(Duration::from_millis(1)) => {}
					}
				}
				active.fetch_sub(1, Ordering::SeqCst);
				let _ = generation_id;
			});

			if count <= remaining_failures {
				Err(format!("deliberate startup failure #{count}"))
			} else {
				Ok(())
			}
		}

		async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
			Ok(ActorFlow::Continue)
		}
	}

	#[tokio::test]
	async fn no_zombie_tickers_across_restarts() {
		let active_tickers = Arc::new(AtomicUsize::new(0));
		let peak_tickers = Arc::new(AtomicUsize::new(0));
		let starts = Arc::new(AtomicUsize::new(0));
		let fail_first_n = Arc::new(AtomicUsize::new(3)); // fail first 3 starts

		let at = Arc::clone(&active_tickers);
		let pt = Arc::clone(&peak_tickers);
		let st = Arc::clone(&starts);
		let ff = Arc::clone(&fail_first_n);

		let handle = spawn_supervised_actor(
			ActorSpec::new("zombie-detector", TaskClass::Background, move || ZombieDetectorActor {
				active_tickers: Arc::clone(&at),
				peak_tickers: Arc::clone(&pt),
				starts: Arc::clone(&st),
				fail_first_n: Arc::clone(&ff),
			})
			.supervisor(SupervisorSpec {
				restart: RestartPolicy::OnFailure {
					max_restarts: 5,
					backoff: Duration::from_millis(1),
				},
				event_buffer: 8,
			}),
		);

		// Wait for restarts to settle (3 failures + 1 success = 4 starts).
		tokio::time::sleep(Duration::from_millis(100)).await;
		assert_eq!(starts.load(Ordering::SeqCst), 4, "should start 4 times (3 failures + 1 success)");

		// After settling, exactly one ticker should be active.
		assert_eq!(active_tickers.load(Ordering::SeqCst), 1, "exactly one ticker should be active after restarts");

		// Shutdown and verify all tickers stop.
		handle.cancel();
		let report = handle.shutdown(ShutdownMode::Immediate).await;
		assert!(report.completed());

		// Give ticker tasks a moment to observe cancellation.
		tokio::time::sleep(Duration::from_millis(20)).await;
		assert_eq!(active_tickers.load(Ordering::SeqCst), 0, "all tickers should stop after shutdown");

		// Peak should reflect zombie accumulation if cancellation is broken.
		// With correct per-generation cancellation, peak should be 1.
		// With broken cancellation (all share parent token), peak could be up to 4.
		let peak = peak_tickers.load(Ordering::SeqCst);
		assert_eq!(peak, 1, "peak concurrent tickers should be 1 (no zombies); got {peak}");
	}

	#[tokio::test]
	async fn shutdown_during_backoff_completes_promptly() {
		let starts = Arc::new(AtomicUsize::new(0));
		let starts_clone = Arc::clone(&starts);

		// Actor that fails on first message, triggering OnFailure restart + backoff.
		struct FailOnceActor;
		#[async_trait]
		impl WorkerActor for FailOnceActor {
			type Cmd = ();
			type Evt = ();
			async fn handle(&mut self, _cmd: (), _ctx: &mut ActorContext<()>) -> Result<ActorFlow, String> {
				Err("deliberate failure".into())
			}
		}

		let handle = spawn_supervised_actor(
			ActorSpec::new("backoff-shutdown", TaskClass::Background, move || {
				let s = Arc::clone(&starts_clone);
				s.fetch_add(1, Ordering::SeqCst);
				FailOnceActor
			})
			.supervisor(SupervisorSpec {
				restart: RestartPolicy::OnFailure {
					max_restarts: 5,
					backoff: Duration::from_secs(60), // very long backoff
				},
				event_buffer: 8,
			}),
		);

		// Trigger failure → supervisor enters 60s backoff sleep.
		let _ = handle.send(()).await;
		tokio::time::sleep(Duration::from_millis(20)).await;

		let starts_before = starts.load(Ordering::SeqCst);
		assert_eq!(starts_before, 1, "only one start so far");

		// Shutdown must complete promptly despite the 60s backoff.
		let report = tokio::time::timeout(Duration::from_millis(500), handle.shutdown(ShutdownMode::Immediate))
			.await
			.expect("shutdown should not hang during backoff");
		assert!(report.completed());

		// No additional restart should have occurred.
		assert_eq!(starts.load(Ordering::SeqCst), 1, "no restart after shutdown during backoff");
	}

	#[tokio::test]
	async fn panic_path_triggers_restart_same_as_error() {
		let starts = Arc::new(AtomicUsize::new(0));

		struct PanicOnStartActor {
			starts: Arc<AtomicUsize>,
		}

		#[async_trait]
		impl WorkerActor for PanicOnStartActor {
			type Cmd = ();
			type Evt = ();

			async fn on_start(&mut self, _ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
				self.starts.fetch_add(1, Ordering::SeqCst);
				panic!("deliberate startup panic");
			}

			async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
				unreachable!();
			}
		}

		let starts_clone = Arc::clone(&starts);
		let handle = spawn_supervised_actor(
			ActorSpec::new("panic-restart", TaskClass::Background, move || PanicOnStartActor {
				starts: Arc::clone(&starts_clone),
			})
			.supervisor(SupervisorSpec {
				restart: RestartPolicy::OnFailure {
					max_restarts: 2,
					backoff: Duration::from_millis(1),
				},
				event_buffer: 8,
			}),
		);

		tokio::time::sleep(Duration::from_millis(100)).await;

		// Should have started 3 times (initial + 2 restarts).
		let total_starts = starts.load(Ordering::SeqCst);
		assert_eq!(total_starts, 3, "panic should trigger same restart logic as error");

		// Final exit should be Panicked.
		let last_exit = handle.last_exit().await;
		assert_eq!(last_exit.as_ref().map(|e| e.kind()), Some(ActorExitKind::Panicked));

		handle.cancel();
		let report = handle.shutdown(ShutdownMode::Immediate).await;
		assert!(report.completed());
	}

	#[tokio::test]
	async fn max_restarts_honored_then_stops() {
		let starts = Arc::new(AtomicUsize::new(0));

		struct AlwaysFailActor {
			starts: Arc<AtomicUsize>,
		}

		#[async_trait]
		impl WorkerActor for AlwaysFailActor {
			type Cmd = ();
			type Evt = ();

			async fn on_start(&mut self, _ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
				let count = self.starts.fetch_add(1, Ordering::SeqCst) + 1;
				Err(format!("fail #{count}"))
			}

			async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
				unreachable!("on_start always fails");
			}
		}

		let starts_clone = Arc::clone(&starts);
		let handle = spawn_supervised_actor(
			ActorSpec::new("max-restarts", TaskClass::Background, move || AlwaysFailActor {
				starts: Arc::clone(&starts_clone),
			})
			.supervisor(SupervisorSpec {
				restart: RestartPolicy::OnFailure {
					max_restarts: 3,
					backoff: Duration::from_millis(1),
				},
				event_buffer: 8,
			}),
		);

		// Wait for all restarts to exhaust.
		tokio::time::sleep(Duration::from_millis(100)).await;

		// initial (1) + 3 restarts = 4 total starts.
		let total = starts.load(Ordering::SeqCst);
		assert_eq!(total, 4, "should start exactly 1 + max_restarts times");

		let last_exit = handle.last_exit().await;
		assert!(
			last_exit.as_ref().map(|e| e.kind()) == Some(ActorExitKind::StartupFailed),
			"final exit should be StartupFailed, got {last_exit:?}"
		);

		// Supervisor should have already exited (no more restarts).
		let report = tokio::time::timeout(Duration::from_millis(100), handle.shutdown(ShutdownMode::Immediate))
			.await
			.expect("shutdown should complete quickly when supervisor already exited");
		assert!(report.completed());
	}

	#[tokio::test]
	async fn generation_advances_on_each_restart() {
		let generations = Arc::new(Mutex::new(Vec::<u64>::new()));

		struct GenTrackingActor {
			generations: Arc<Mutex<Vec<u64>>>,
		}

		#[async_trait]
		impl WorkerActor for GenTrackingActor {
			type Cmd = ();
			type Evt = ();

			async fn on_start(&mut self, ctx: &mut ActorContext<Self::Evt>) -> Result<(), String> {
				self.generations.lock().await.push(ctx.generation());
				Err("fail".to_string())
			}

			async fn handle(&mut self, _cmd: Self::Cmd, _ctx: &mut ActorContext<Self::Evt>) -> Result<ActorFlow, String> {
				unreachable!();
			}
		}

		let gens = Arc::clone(&generations);
		let handle = spawn_supervised_actor(
			ActorSpec::new("gen-tracking", TaskClass::Background, move || GenTrackingActor {
				generations: Arc::clone(&gens),
			})
			.supervisor(SupervisorSpec {
				restart: RestartPolicy::OnFailure {
					max_restarts: 3,
					backoff: Duration::from_millis(1),
				},
				event_buffer: 8,
			}),
		);

		tokio::time::sleep(Duration::from_millis(100)).await;
		handle.cancel();
		let _ = handle.shutdown(ShutdownMode::Immediate).await;

		let gens = generations.lock().await;
		assert_eq!(gens.len(), 4, "4 starts = 4 generations");

		// Generations must be strictly monotonically increasing.
		for window in gens.windows(2) {
			assert!(window[1] > window[0], "generations must be strictly increasing: {gens:?}");
		}

		// Handle's generation() should match the last one.
		assert_eq!(handle.generation(), *gens.last().unwrap());
	}
}
