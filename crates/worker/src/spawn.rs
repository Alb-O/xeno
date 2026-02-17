use std::future::Future;

use tokio::task::JoinHandle;

use crate::TaskClass;

/// Spawns an async task with shared worker classification metadata.
///
/// Must be called from within an active Tokio runtime context.
pub fn spawn<F>(class: TaskClass, fut: F) -> JoinHandle<F::Output>
where
	F: Future + Send + 'static,
	F::Output: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn");
	tokio::spawn(fut)
}

/// Spawns blocking work with shared worker classification metadata.
///
/// Must be called from within an active Tokio runtime context.
pub fn spawn_blocking<F, R>(class: TaskClass, f: F) -> JoinHandle<R>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_blocking");
	tokio::task::spawn_blocking(f)
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
