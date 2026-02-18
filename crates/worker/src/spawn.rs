use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::JoinHandle;

use crate::TaskClass;

fn fallback_runtime() -> &'static Runtime {
	static FALLBACK_RUNTIME: OnceLock<Runtime> = OnceLock::new();
	FALLBACK_RUNTIME.get_or_init(|| {
		Builder::new_multi_thread()
			.worker_threads(2)
			.thread_name("xeno-worker-fallback")
			.enable_all()
			.build()
			.expect("worker fallback runtime must initialize")
	})
}

pub(crate) fn current_or_fallback_handle() -> Handle {
	Handle::try_current().unwrap_or_else(|_| fallback_runtime().handle().clone())
}

/// Spawns an async task with shared worker classification metadata.
///
/// This is the only sanctioned entry point for `tokio::spawn` in the workspace.
#[allow(clippy::disallowed_methods)]
pub fn spawn<F>(class: TaskClass, fut: F) -> JoinHandle<F::Output>
where
	F: Future + Send + 'static,
	F::Output: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn");
	current_or_fallback_handle().spawn(fut)
}

/// Spawns blocking work with shared worker classification metadata.
///
/// This is the only sanctioned entry point for `tokio::task::spawn_blocking` in the workspace.
#[allow(clippy::disallowed_methods)]
pub fn spawn_blocking<F, R>(class: TaskClass, f: F) -> JoinHandle<R>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_blocking");
	current_or_fallback_handle().spawn_blocking(f)
}

/// Spawns a dedicated OS thread with shared worker classification metadata.
///
/// This is the only sanctioned entry point for `std::thread::spawn` in the workspace.
#[allow(clippy::disallowed_methods)]
pub fn spawn_thread<F, R>(class: TaskClass, f: F) -> std::thread::JoinHandle<R>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_thread");
	std::thread::spawn(f)
}

/// Spawns a dedicated named OS thread with shared worker classification metadata.
///
/// This is the only sanctioned entry point for `std::thread::Builder::spawn` in the workspace.
#[allow(clippy::disallowed_methods)]
pub fn spawn_named_thread<F, R>(class: TaskClass, name: impl Into<String>, f: F) -> std::io::Result<std::thread::JoinHandle<R>>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_named_thread");
	std::thread::Builder::new().name(name.into()).spawn(f)
}
