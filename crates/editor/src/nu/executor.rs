//! Persistent Nu worker actor for hook and macro evaluation.
//!
//! Uses `xeno-worker` supervision + mailbox policies instead of custom
//! thread/channel lifecycle management.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::oneshot;
use xeno_invocation::nu::DecodeBudget;
use xeno_nu_api::ExportId;
use xeno_nu_data::Value;

use super::{NuDecodeSurface, NuEffectBatch, NuRuntime};

/// A command sent to the supervised Nu worker actor.
enum Job {
	Run {
		decl_id: ExportId,
		surface: NuDecodeSurface,
		args: Vec<String>,
		budget: DecodeBudget,
		env: Vec<(String, Value)>,
		host: Option<Box<dyn xeno_nu_api::XenoNuHost + Send>>,
		span: tracing::Span,
		reply: oneshot::Sender<Result<NuEffectBatch, JobError>>,
	},
}

#[derive(Debug)]
enum JobError {
	Eval(String),
	Transport(String),
}

struct NuActor {
	runtime: NuRuntime,
}

#[async_trait::async_trait]
impl xeno_worker::Actor for NuActor {
	type Cmd = Job;
	type Evt = ();

	async fn handle(&mut self, cmd: Self::Cmd, _ctx: &mut xeno_worker::ActorContext<Self::Evt>) -> Result<xeno_worker::ActorFlow, String> {
		match cmd {
			Job::Run {
				decl_id,
				surface,
				args,
				budget,
				env,
				host,
				span,
				reply,
			} => {
				let runtime = self.runtime.clone();
				let result = xeno_worker::spawn::spawn_blocking(xeno_worker::TaskClass::CpuBlocking, move || {
					let _guard = span.enter();
					let host_ref = host.as_ref().map(|h| h.as_ref() as &(dyn xeno_nu_api::XenoNuHost + 'static));
					std::panic::catch_unwind(AssertUnwindSafe(|| {
						runtime.run_effects_by_decl_id_owned(decl_id, surface, args, budget, env, host_ref)
					}))
				})
				.await;

				match result {
					Ok(Ok(Ok(batch))) => {
						let _ = reply.send(Ok(batch));
						Ok(xeno_worker::ActorFlow::Continue)
					}
					Ok(Ok(Err(eval_error))) => {
						let _ = reply.send(Err(JobError::Eval(eval_error)));
						Ok(xeno_worker::ActorFlow::Continue)
					}
					Ok(Err(_panic)) => {
						let _ = reply.send(Err(JobError::Transport("Nu executor panicked during evaluation".to_string())));
						Err("nu actor panic".to_string())
					}
					Err(join_error) => {
						let reason = format!("Nu executor join failure: {join_error}");
						let _ = reply.send(Err(JobError::Transport(reason.clone())));
						Err(reason)
					}
				}
			}
		}
	}
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
	_runtime_guard: Option<Arc<tokio::runtime::Runtime>>,
	handle: Arc<xeno_worker::ActorHandle<Job, ()>>,
	closed: AtomicBool,
	#[cfg(test)]
	pub(crate) shutdown_acks: AtomicUsize,
}

/// Handle to a supervised Nu evaluation worker actor.
///
/// Supports owner/client semantics: the original `NuExecutor` is the owner.
/// Client clones can submit jobs but do not trigger shutdown on drop.
pub struct NuExecutor {
	shared: Arc<Shared>,
	is_owner: bool,
}

impl NuExecutor {
	fn build_worker_runtime() -> tokio::runtime::Runtime {
		tokio::runtime::Builder::new_multi_thread()
			.enable_all()
			.worker_threads(2)
			.thread_name("xeno-nu-worker")
			.build()
			.expect("failed to build Nu worker runtime")
	}

	/// Spawn a new worker actor for the given runtime.
	pub fn new(runtime: NuRuntime) -> Self {
		let actor_runtime = runtime.clone();
		let spec = xeno_worker::ActorSpec::new("nu.executor", xeno_worker::TaskClass::CpuBlocking, move || NuActor {
			runtime: actor_runtime.clone(),
		})
		.mailbox(xeno_worker::ActorMailboxPolicy {
			capacity: 256,
			policy: xeno_worker::ActorMailboxMode::Backpressure,
		})
		.supervisor(xeno_worker::ActorLifecyclePolicy {
			restart: xeno_worker::ActorRestartPolicy::OnFailure {
				max_restarts: 8,
				backoff: std::time::Duration::from_millis(25),
			},
			event_buffer: 8,
		});

		let (runtime_guard, handle) = if tokio::runtime::Handle::try_current().is_ok() {
			(None, Arc::new(xeno_worker::ActorRuntime::spawn(spec)))
		} else {
			let rt = Arc::new(Self::build_worker_runtime());
			let actor = Arc::new(rt.block_on(async move { xeno_worker::ActorRuntime::spawn(spec) }));
			(Some(rt), actor)
		};

		Self {
			shared: Arc::new(Shared {
				_runtime_guard: runtime_guard,
				handle,
				closed: AtomicBool::new(false),
				#[cfg(test)]
				shutdown_acks: AtomicUsize::new(0),
			}),
			is_owner: true,
		}
	}

	/// Creates a non-owning client clone that shares the worker actor.
	pub fn client(&self) -> Self {
		Self {
			shared: Arc::clone(&self.shared),
			is_owner: false,
		}
	}

	/// Submit a job and await the result.
	pub async fn run(
		&self,
		decl_id: ExportId,
		surface: NuDecodeSurface,
		args: Vec<String>,
		budget: DecodeBudget,
		env: Vec<(String, Value)>,
		host: Option<Box<dyn xeno_nu_api::XenoNuHost + Send>>,
	) -> Result<NuEffectBatch, NuExecError> {
		if self.shared.closed.load(Ordering::SeqCst) {
			return Err(NuExecError::Closed);
		}

		let (reply_tx, reply_rx) = oneshot::channel();
		let send_result = self
			.shared
			.handle
			.send(Job::Run {
				decl_id,
				surface,
				args,
				budget,
				env,
				host,
				span: tracing::Span::current(),
				reply: reply_tx,
			})
			.await;
		if let Err(err) = send_result {
			if self.shared.closed.load(Ordering::SeqCst) {
				return Err(NuExecError::Closed);
			}
			return Err(NuExecError::Transport(format!("channel send failed (worker unavailable): {err:?}")));
		}

		match reply_rx.await {
			Ok(Ok(batch)) => Ok(batch),
			Ok(Err(JobError::Eval(msg))) => Err(NuExecError::Eval(msg)),
			Ok(Err(JobError::Transport(msg))) => Err(NuExecError::Transport(msg)),
			Err(_) => Err(NuExecError::Transport("reply channel dropped (worker stopped mid-evaluation)".to_string())),
		}
	}

	#[cfg(test)]
	pub(crate) fn shutdown_acks_for_tests(&self) -> Arc<Shared> {
		Arc::clone(&self.shared)
	}
}

impl Drop for NuExecutor {
	fn drop(&mut self) {
		if !self.is_owner {
			return;
		}

		self.shared.closed.store(true, Ordering::SeqCst);
		self.shared.handle.cancel();
		#[cfg(test)]
		self.shared.shutdown_acks.fetch_add(1, Ordering::SeqCst);
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
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats }");
		let decl_id = runtime.find_export("go").expect("go should exist");
		let executor = NuExecutor::new(runtime);

		let result = executor
			.run(decl_id, NuDecodeSurface::Macro, vec![], DecodeBudget::macro_defaults(), vec![], None)
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
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats }");
		let executor = NuExecutor::new(runtime);
		let shared = Arc::clone(&executor.shared);

		drop(executor);

		assert_eq!(shared.shutdown_acks.load(Ordering::SeqCst), 1, "drop should receive shutdown ack");
		assert!(shared.closed.load(Ordering::SeqCst), "closed flag should be set");
	}

	#[tokio::test]
	async fn closed_executor_returns_closed_error() {
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats }");
		let decl_id = runtime.find_export("go").expect("go should exist");
		let executor = NuExecutor::new(runtime);
		let client = executor.client();

		drop(executor); // closes

		let result = client
			.run(decl_id, NuDecodeSurface::Macro, vec![], DecodeBudget::macro_defaults(), vec![], None)
			.await;

		assert!(matches!(result, Err(NuExecError::Closed)));
	}

	#[test]
	fn client_clone_does_not_send_shutdown() {
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats }");
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
