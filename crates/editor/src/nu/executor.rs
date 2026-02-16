//! Persistent Nu worker thread for hook and macro evaluation.
//!
//! Replaces per-call `spawn_blocking` with a dedicated thread that owns the
//! `NuRuntime` and processes jobs sequentially. This eliminates tokio blocking
//! pool scheduling overhead on the hot path (every action with a post-hook).
//!
//! Caller tracing spans are propagated into the worker thread so that logs
//! emitted during Nu evaluation appear nested under the originating
//! `nu.hook` / `nu.macro` span.
//!
//! Shutdown is explicit and deterministic: dropping `NuExecutor` sends a
//! `Shutdown` job and waits briefly for an ack so runtime swaps do not leave
//! stale workers alive. If the worker panics during evaluation, it sends an
//! error reply and exits; the editor-side retry logic recreates the executor
//! and retries once using the payload returned in [`NuExecError::Shutdown`].

use std::panic::AssertUnwindSafe;
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use nu_protocol::{DeclId, Value};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use xeno_invocation::nu::DecodeLimits;

use super::NuRuntime;
use crate::types::Invocation;

/// A job sent to the Nu worker thread.
enum Job {
	Run {
		decl_id: DeclId,
		args: Vec<String>,
		limits: DecodeLimits,
		env: Vec<(String, Value)>,
		span: tracing::Span,
		reply: oneshot::Sender<Result<Vec<Invocation>, String>>,
	},
	Shutdown {
		ack: oneshot::Sender<()>,
	},
}

/// Executor error distinguishing transport failures (payload recoverable)
/// from Nu evaluation errors.
#[derive(Debug)]
pub enum NuExecError {
	/// Channel send failed — receiver is gone. Payload returned for retry.
	Shutdown {
		decl_id: DeclId,
		args: Vec<String>,
		limits: DecodeLimits,
		env: Vec<(String, Value)>,
	},
	/// Job was enqueued but the reply channel was dropped (worker died
	/// mid-evaluation). Payload is lost.
	ReplyDropped,
	/// Nu evaluated and returned an error string.
	Eval(String),
}

/// Handle to a persistent Nu evaluation thread.
///
/// Sending jobs through `run` dispatches them to the worker. The worker
/// processes jobs sequentially using the `NuRuntime` it owns.
///
/// Supports owner/client semantics: the original `NuExecutor` is the owner
/// and sends a `Shutdown` job on drop. Clones are clients that share the
/// same channel but do not trigger shutdown when dropped.
pub struct NuExecutor {
	tx: std::sync::mpsc::Sender<Job>,
	/// Only the owner sends `Shutdown` on drop.
	is_owner: bool,
	#[cfg(test)]
	shutdown_acks: Arc<AtomicUsize>,
}

impl NuExecutor {
	/// Spawn a new worker thread for the given runtime.
	pub fn new(runtime: NuRuntime) -> Self {
		let (tx, rx) = std::sync::mpsc::channel::<Job>();
		#[cfg(test)]
		let shutdown_acks = Arc::new(AtomicUsize::new(0));

		thread::Builder::new()
			.name("nu-executor".into())
			.spawn(move || {
				while let Ok(job) = rx.recv() {
					match job {
						Job::Run {
							decl_id,
							args,
							limits,
							env,
							span,
							reply,
						} => {
							let _guard = span.enter();
							let result = std::panic::catch_unwind(AssertUnwindSafe(|| runtime.run_invocations_by_decl_id_owned(decl_id, args, limits, env)));
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

		Self {
			tx,
			is_owner: true,
			#[cfg(test)]
			shutdown_acks,
		}
	}

	/// Creates a non-owning client clone that shares the worker channel.
	///
	/// Client clones can submit jobs but do not send `Shutdown` on drop.
	/// This is safe to move into `'static + Send` futures for background
	/// hook evaluation.
	pub fn client(&self) -> Self {
		Self {
			tx: self.tx.clone(),
			is_owner: false,
			#[cfg(test)]
			shutdown_acks: Arc::clone(&self.shutdown_acks),
		}
	}

	/// Submit a job and await the result.
	///
	/// Captures the current tracing span and propagates it into the worker
	/// thread so that logs appear under the caller's span hierarchy.
	///
	/// On transport failure ([`NuExecError::Shutdown`]), the original payload
	/// is returned so the caller can retry without cloning.
	pub async fn run(&self, decl_id: DeclId, args: Vec<String>, limits: DecodeLimits, env: Vec<(String, Value)>) -> Result<Vec<Invocation>, NuExecError> {
		let (reply_tx, reply_rx) = oneshot::channel();

		if let Err(err) = self.tx.send(Job::Run {
			decl_id,
			args,
			limits,
			env,
			span: tracing::Span::current(),
			reply: reply_tx,
		}) {
			match err.0 {
				Job::Run {
					decl_id, args, limits, env, ..
				} => {
					return Err(NuExecError::Shutdown { decl_id, args, limits, env });
				}
				Job::Shutdown { .. } => unreachable!("shutdown jobs are only sent from Drop"),
			}
		}

		match reply_rx.await {
			Ok(Ok(invocations)) => Ok(invocations),
			Ok(Err(eval_error)) => Err(NuExecError::Eval(eval_error)),
			Err(_) => Err(NuExecError::ReplyDropped),
		}
	}

	#[cfg(test)]
	fn from_sender(tx: std::sync::mpsc::Sender<Job>) -> Self {
		Self {
			tx,
			is_owner: true,
			shutdown_acks: Arc::new(AtomicUsize::new(0)),
		}
	}

	#[cfg(test)]
	pub(crate) fn shutdown_acks_for_tests(&self) -> Arc<AtomicUsize> {
		Arc::clone(&self.shutdown_acks)
	}
}

const SHUTDOWN_ACK_TIMEOUT: Duration = Duration::from_millis(100);

impl Drop for NuExecutor {
	fn drop(&mut self) {
		if !self.is_owner {
			return;
		}

		let (ack_tx, mut ack_rx) = oneshot::channel();
		if self.tx.send(Job::Shutdown { ack: ack_tx }).is_err() {
			return;
		}

		let deadline = Instant::now() + SHUTDOWN_ACK_TIMEOUT;
		loop {
			match ack_rx.try_recv() {
				Ok(()) => {
					#[cfg(test)]
					self.shutdown_acks.fetch_add(1, Ordering::SeqCst);
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

	fn make_runtime(script: &str) -> NuRuntime {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		std::fs::write(temp.path().join("xeno.nu"), script).expect("write should succeed");
		let path = temp.keep();
		NuRuntime::load(&path).expect("runtime should load")
	}

	#[tokio::test]
	async fn executor_runs_invocations() {
		let runtime = make_runtime("export def go [] { editor stats }");
		let decl_id = runtime.find_script_decl("go").expect("go should exist");
		let executor = NuExecutor::new(runtime);

		let result = executor
			.run(decl_id, vec![], DecodeLimits::macro_defaults(), vec![])
			.await
			.expect("run should succeed");

		assert!(!result.is_empty());
		assert!(matches!(result.as_slice(), [Invocation::EditorCommand { name, .. }] if name == "stats"));
	}

	#[test]
	fn executor_shutdown_on_drop() {
		let runtime = make_runtime("export def go [] { editor stats }");
		let executor = NuExecutor::new(runtime);
		let shutdown_acks = executor.shutdown_acks_for_tests();

		drop(executor);

		assert_eq!(shutdown_acks.load(Ordering::SeqCst), 1, "drop should receive shutdown ack");
	}

	#[tokio::test]
	async fn shutdown_returns_payload() {
		let (tx, rx) = std::sync::mpsc::channel::<Job>();
		drop(rx); // no receiver — immediate send failure

		let executor = NuExecutor::from_sender(tx);
		let args = vec!["a".to_string(), "b".to_string()];
		let env = vec![("KEY".to_string(), Value::test_string("val"))];

		let result = executor.run(DeclId::new(42), args, DecodeLimits::macro_defaults(), env).await;

		match result {
			Err(NuExecError::Shutdown { decl_id, args, env, .. }) => {
				assert_eq!(decl_id, DeclId::new(42));
				assert_eq!(args, vec!["a".to_string(), "b".to_string()]);
				assert_eq!(env.len(), 1);
				assert_eq!(env[0].0, "KEY");
			}
			other => panic!("expected Shutdown, got {:?}", other.is_ok()),
		}
	}

	#[test]
	fn client_clone_does_not_send_shutdown() {
		let runtime = make_runtime("export def go [] { editor stats }");
		let executor = NuExecutor::new(runtime);
		let shutdown_acks = executor.shutdown_acks_for_tests();

		let client = executor.client();
		drop(client);

		// Client drop must not send shutdown.
		assert_eq!(shutdown_acks.load(Ordering::SeqCst), 0, "client drop should not send shutdown");

		// Owner drop sends shutdown.
		drop(executor);
		assert_eq!(shutdown_acks.load(Ordering::SeqCst), 1, "owner drop should send shutdown");
	}

	/// Compile-time proof that `tracing::Span` is `Send`.
	#[allow(dead_code)]
	fn assert_span_is_send() {
		fn require_send<T: Send>() {}
		require_send::<tracing::Span>();
	}
}
