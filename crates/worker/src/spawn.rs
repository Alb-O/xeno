use std::future::Future;
use std::sync::OnceLock;

use tokio::task::JoinHandle;

use crate::TaskClass;

fn runtime_handle() -> tokio::runtime::Handle {
	if let Ok(handle) = tokio::runtime::Handle::try_current() {
		return handle;
	}

	static GLOBAL_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
	let runtime = GLOBAL_RT.get_or_init(|| {
		tokio::runtime::Builder::new_multi_thread()
			.enable_all()
			.worker_threads(2)
			.thread_name("xeno-worker-global")
			.build()
			.expect("failed to build xeno-worker global tokio runtime")
	});
	runtime.handle().clone()
}

/// Spawns an async task with shared worker classification metadata.
pub fn spawn<F>(class: TaskClass, fut: F) -> JoinHandle<F::Output>
where
	F: Future + Send + 'static,
	F::Output: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn");
	runtime_handle().spawn(fut)
}

/// Spawns blocking work with shared worker classification metadata.
pub fn spawn_blocking<F, R>(class: TaskClass, f: F) -> JoinHandle<R>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_blocking");
	runtime_handle().spawn_blocking(f)
}

/// Spawns a dedicated OS thread with shared worker classification metadata.
pub fn spawn_thread<F, R>(class: TaskClass, f: F) -> std::thread::JoinHandle<R>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_thread");
	std::thread::spawn(f)
}

/// Spawns a dedicated named OS thread with shared worker classification metadata.
pub fn spawn_named_thread<F, R>(class: TaskClass, name: impl Into<String>, f: F) -> std::io::Result<std::thread::JoinHandle<R>>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_named_thread");
	std::thread::Builder::new().name(name.into()).spawn(f)
}
