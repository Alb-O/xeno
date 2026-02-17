//! Persistent Nu worker thread for hook and macro evaluation.
//!
//! Replaces per-call `spawn_blocking` with a dedicated thread that owns the
//! `NuRuntime` and processes jobs sequentially. This eliminates tokio blocking
//! pool scheduling overhead on the hot path (every action with a post-hook).
//!
//! The executor self-heals on recoverable transport failures: when the worker
//! channel send fails, `run()` performs one internal restart and retries the
//! same job payload. If the worker dies mid-evaluation and drops the reply
//! channel, payload replay is unsafe and `run()` returns transport error after
//! attempting to respawn for future calls.
//!
//! Shutdown is explicit and deterministic: dropping the owning `NuExecutor`
//! sets a closed flag and sends a `Shutdown` job. Once closed, no restarts
//! are allowed — stale client clones from previous runtime epochs get
//! `NuExecError::Closed`.

use std::panic::AssertUnwindSafe;
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use xeno_invocation::nu::DecodeBudget;
use xeno_nu_api::ExportId;
use xeno_nu_data::Value;

use super::{NuDecodeSurface, NuEffectBatch, NuRuntime};

/// A job sent to the Nu worker thread.
enum Job {
	Run {
		decl_id: ExportId,
		surface: NuDecodeSurface,
		args: Vec<String>,
		budget: DecodeBudget,
		env: Vec<(String, Value)>,
		span: tracing::Span,
		reply: oneshot::Sender<Result<NuEffectBatch, String>>,
	},
	Shutdown {
		ack: oneshot::Sender<()>,
	},
}

/// Executor error surfaced by the persistent Nu worker.
#[derive(Debug)]
pub enum NuExecError {
	/// Owner dropped or runtime was swapped; no restart allowed.
	Closed,
	/// Transport failure not recoverable for this call.
	Transport(String),
	/// Nu evaluated and returned an error string.
	Eval(String),
}

/// Shared state between owner and client clones.
pub(crate) struct Shared {
	runtime: NuRuntime,
	tx: std::sync::Mutex<std::sync::mpsc::Sender<Job>>,
	closed: AtomicBool,
	#[cfg(test)]
	pub(crate) shutdown_acks: AtomicUsize,
}

/// Handle to a persistent Nu evaluation thread.
///
/// Sending jobs through `run` dispatches them to the worker. The worker
/// processes jobs sequentially using the `NuRuntime` it owns.
///
/// Supports owner/client semantics: the original `NuExecutor` is the owner
/// and sends a `Shutdown` job on drop. Clones are clients that share the
/// same channel but do not trigger shutdown when dropped.
///
/// Self-heals on recoverable transport failures: if send fails, `run()`
/// respawns once and retries the same payload. After the owner is dropped
/// (`closed=true`), no restarts are allowed.
pub struct NuExecutor {
	shared: std::sync::Arc<Shared>,
	is_owner: bool,
}

impl NuExecutor {
	/// Spawn a new worker thread for the given runtime.
	pub fn new(runtime: NuRuntime) -> Self {
		let (tx, rx) = std::sync::mpsc::channel::<Job>();
		let rt = runtime.clone();

		Self::spawn_worker(rt, rx);

		Self {
			shared: std::sync::Arc::new(Shared {
				runtime,
				tx: std::sync::Mutex::new(tx),
				closed: AtomicBool::new(false),
				#[cfg(test)]
				shutdown_acks: AtomicUsize::new(0),
			}),
			is_owner: true,
		}
	}

	fn spawn_worker(runtime: NuRuntime, rx: std::sync::mpsc::Receiver<Job>) {
		xeno_worker::spawn_named_thread(xeno_worker::TaskClass::CpuBlocking, "nu-executor", move || {
			while let Ok(job) = rx.recv() {
				match job {
					Job::Run {
						decl_id,
						surface,
						args,
						budget,
						env,
						span,
						reply,
					} => {
						let _guard = span.enter();
						let result = std::panic::catch_unwind(AssertUnwindSafe(|| runtime.run_effects_by_decl_id_owned(decl_id, surface, args, budget, env)));
						match result {
							Ok(value) => {
								let _ = reply.send(value);
							}
							Err(_) => {
								let _ = reply.send(Err("Nu executor panicked during evaluation".to_string()));
								break;
							}
						}
					}
					Job::Shutdown { ack } => {
						let _ = ack.send(());
						break;
					}
				}
			}
		})
		.expect("failed to spawn nu-executor thread");
	}

	/// Respawn the worker thread. Returns false if closed.
	fn respawn(&self) -> bool {
		if self.shared.closed.load(Ordering::SeqCst) {
			return false;
		}
		let (new_tx, rx) = std::sync::mpsc::channel::<Job>();
		Self::spawn_worker(self.shared.runtime.clone(), rx);
		*self.shared.tx.lock().expect("tx lock poisoned") = new_tx;
		true
	}

	/// Creates a non-owning client clone that shares the worker channel.
	///
	/// Client clones can submit jobs but do not send `Shutdown` on drop.
	/// This is safe to move into `'static + Send` futures for background
	/// hook evaluation.
	pub fn client(&self) -> Self {
		Self {
			shared: std::sync::Arc::clone(&self.shared),
			is_owner: false,
		}
	}

	/// Submit a job and await the result.
	///
	/// On recoverable transport failure, restarts once and retries the same job.
	/// If payload replay is unsafe (reply dropped), returns transport error.
	/// If the executor is closed (owner dropped), returns `NuExecError::Closed`.
	pub async fn run(
		&self,
		decl_id: ExportId,
		surface: NuDecodeSurface,
		args: Vec<String>,
		budget: DecodeBudget,
		env: Vec<(String, Value)>,
	) -> Result<NuEffectBatch, NuExecError> {
		if self.shared.closed.load(Ordering::SeqCst) {
			return Err(NuExecError::Closed);
		}

		match self.try_run(decl_id, surface, args, budget, env).await {
			Ok(batch) => Ok(batch),
			Err(TransportOrEval::Eval(e)) => Err(NuExecError::Eval(e)),
			Err(TransportOrEval::Transport {
				payload: Some(RetryPayload {
					decl_id,
					surface,
					args,
					budget,
					env,
				}),
				reason,
			}) => {
				tracing::warn!(reason = %reason, "Nu executor transport failure, attempting restart");
				if !self.respawn() {
					return Err(NuExecError::Closed);
				}
				// Retry once after restart.
				match self.try_run(decl_id, surface, args, budget, env).await {
					Ok(batch) => Ok(batch),
					Err(TransportOrEval::Eval(e)) => Err(NuExecError::Eval(e)),
					Err(TransportOrEval::Transport { reason, .. }) => Err(NuExecError::Transport(reason)),
				}
			}
			Err(TransportOrEval::Transport { payload: None, reason }) => {
				// Payload was lost mid-flight (reply channel dropped), so replay is not safe.
				let _ = self.respawn();
				Err(NuExecError::Transport(reason))
			}
		}
	}

	async fn try_run(
		&self,
		decl_id: ExportId,
		surface: NuDecodeSurface,
		args: Vec<String>,
		budget: DecodeBudget,
		env: Vec<(String, Value)>,
	) -> Result<NuEffectBatch, TransportOrEval> {
		let (reply_tx, reply_rx) = oneshot::channel();

		{
			let tx = self.shared.tx.lock().expect("tx lock poisoned");
			if let Err(err) = tx.send(Job::Run {
				decl_id,
				surface,
				args,
				budget,
				env,
				span: tracing::Span::current(),
				reply: reply_tx,
			}) {
				match err.0 {
					Job::Run {
						decl_id,
						surface,
						args,
						budget,
						env,
						..
					} => {
						return Err(TransportOrEval::Transport {
							payload: Some(RetryPayload {
								decl_id,
								surface,
								args,
								budget,
								env,
							}),
							reason: "channel send failed (worker dead)".to_string(),
						});
					}
					Job::Shutdown { .. } => unreachable!("shutdown jobs are only sent from Drop"),
				}
			}
		}

		match reply_rx.await {
			Ok(Ok(batch)) => Ok(batch),
			Ok(Err(eval_error)) => Err(TransportOrEval::Eval(eval_error)),
			Err(_) => Err(TransportOrEval::Transport {
				payload: None, // payload lost when reply dropped
				reason: "reply channel dropped (worker died mid-evaluation)".to_string(),
			}),
		}
	}

	#[cfg(test)]
	pub(crate) fn shutdown_acks_for_tests(&self) -> std::sync::Arc<Shared> {
		std::sync::Arc::clone(&self.shared)
	}
}

/// Internal error type for try_run: distinguishes recoverable transport
/// failures (payload retained for retry), non-replayable transport failures,
/// and evaluation errors.
enum TransportOrEval {
	Transport { payload: Option<RetryPayload>, reason: String },
	Eval(String),
}

struct RetryPayload {
	decl_id: ExportId,
	surface: NuDecodeSurface,
	args: Vec<String>,
	budget: DecodeBudget,
	env: Vec<(String, Value)>,
}

const SHUTDOWN_ACK_TIMEOUT: Duration = Duration::from_millis(100);

impl Drop for NuExecutor {
	fn drop(&mut self) {
		if !self.is_owner {
			return;
		}

		self.shared.closed.store(true, Ordering::SeqCst);

		let (ack_tx, mut ack_rx) = oneshot::channel();
		let tx = self.shared.tx.lock().expect("tx lock poisoned");
		if tx.send(Job::Shutdown { ack: ack_tx }).is_err() {
			return;
		}
		drop(tx);

		let deadline = Instant::now() + SHUTDOWN_ACK_TIMEOUT;
		loop {
			match ack_rx.try_recv() {
				Ok(()) => {
					#[cfg(test)]
					self.shared.shutdown_acks.fetch_add(1, Ordering::SeqCst);
					return;
				}
				Err(TryRecvError::Empty) => {
					if Instant::now() >= deadline {
						return;
					}
					thread::yield_now();
				}
				Err(TryRecvError::Closed) => return,
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::nu::NuEffect;

	fn make_runtime(script: &str) -> NuRuntime {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		std::fs::write(temp.path().join("xeno.nu"), script).expect("write should succeed");
		let path = temp.keep();
		NuRuntime::load(&path).expect("runtime should load")
	}

	#[tokio::test]
	async fn executor_runs_effects() {
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats | xeno effects normalize }");
		let decl_id = runtime.find_script_decl("go").expect("go should exist");
		let executor = NuExecutor::new(runtime);

		let result = executor
			.run(decl_id, NuDecodeSurface::Macro, vec![], DecodeBudget::macro_defaults(), vec![])
			.await
			.expect("run should succeed");

		assert_eq!(result.effects.len(), 1);
		assert!(matches!(
			result.effects.as_slice(),
			[NuEffect::Dispatch(crate::types::Invocation::Command(xeno_invocation::CommandInvocation {
				name,
				route: xeno_invocation::CommandRoute::Editor,
				..
			}))] if name == "stats"
		));
	}

	#[test]
	fn executor_shutdown_on_drop() {
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats | xeno effects normalize }");
		let executor = NuExecutor::new(runtime);
		let shared = Arc::clone(&executor.shared);

		drop(executor);

		assert_eq!(shared.shutdown_acks.load(Ordering::SeqCst), 1, "drop should receive shutdown ack");
		assert!(shared.closed.load(Ordering::SeqCst), "closed flag should be set");
	}

	#[tokio::test]
	async fn closed_executor_returns_closed_error() {
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats | xeno effects normalize }");
		let decl_id = runtime.find_script_decl("go").expect("go should exist");
		let executor = NuExecutor::new(runtime);
		let client = executor.client();

		drop(executor); // closes

		let result = client
			.run(decl_id, NuDecodeSurface::Macro, vec![], DecodeBudget::macro_defaults(), vec![])
			.await;

		assert!(matches!(result, Err(NuExecError::Closed)));
	}

	#[test]
	fn client_clone_does_not_send_shutdown() {
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats | xeno effects normalize }");
		let executor = NuExecutor::new(runtime);
		let shared = Arc::clone(&executor.shared);

		let client = executor.client();
		drop(client);

		// Client drop must not send shutdown.
		assert_eq!(shared.shutdown_acks.load(Ordering::SeqCst), 0, "client drop should not send shutdown");

		// Owner drop sends shutdown.
		drop(executor);
		assert_eq!(shared.shutdown_acks.load(Ordering::SeqCst), 1, "owner drop should send shutdown");
	}

	/// Compile-time proof that `tracing::Span` is `Send`.
	#[allow(dead_code)]
	fn assert_span_is_send() {
		fn require_send<T: Send>() {}
		require_send::<tracing::Span>();
	}
}
