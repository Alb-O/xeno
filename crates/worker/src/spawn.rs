use std::future::Future;

use tokio::runtime::Handle;

use crate::TaskClass;

/// Returns the current tokio runtime handle.
///
/// Panics if called from outside a tokio runtime context. All worker spawning
/// must occur within an active runtime — there is no silent fallback.
pub(crate) fn current_handle() -> Handle {
	Handle::current()
}

/// Spawns an async task with shared worker classification metadata.
///
/// This is the only sanctioned entry point for `tokio::spawn` in the workspace.
/// Panics if called outside a tokio runtime context.
#[allow(clippy::disallowed_methods)]
pub fn spawn<F>(class: TaskClass, fut: F) -> crate::TaskHandle<F::Output>
where
	F: Future + Send + 'static,
	F::Output: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn");
	current_handle().spawn(fut)
}

/// Spawns blocking work with shared worker classification metadata.
///
/// This is the only sanctioned entry point for `tokio::task::spawn_blocking` in the workspace.
/// Panics if called outside a tokio runtime context.
#[allow(clippy::disallowed_methods)]
pub fn spawn_blocking<F, R>(class: TaskClass, f: F) -> crate::TaskHandle<R>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_blocking");
	current_handle().spawn_blocking(f)
}

/// Spawns a dedicated OS thread with shared worker classification metadata.
///
/// This is the only sanctioned entry point for `std::thread::spawn` in the workspace.
#[allow(clippy::disallowed_methods)]
pub fn spawn_thread<F, R>(class: TaskClass, f: F) -> crate::ThreadHandle<R>
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
pub fn spawn_named_thread<F, R>(class: TaskClass, name: impl Into<String>, f: F) -> std::io::Result<crate::ThreadHandle<R>>
where
	F: FnOnce() -> R + Send + 'static,
	R: Send + 'static,
{
	tracing::trace!(worker_class = class.as_str(), "worker.spawn_named_thread");
	std::thread::Builder::new().name(name.into()).spawn(f)
}
