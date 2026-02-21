use std::future::Future;

use tokio::task::{JoinError, JoinSet};

use crate::TaskClass;

/// Reactor-safe wrapper for a Tokio [`JoinSet`].
///
/// Task spawning is routed through `xeno_worker` runtime entry so tasks are
/// attached to the active worker runtime context.
#[derive(Debug)]
pub struct WorkerJoinSet<T> {
	class: TaskClass,
	inner: JoinSet<T>,
}

impl<T> WorkerJoinSet<T>
where
	T: Send + 'static,
{
	/// Creates an empty worker join set for the given task class.
	pub fn new(class: TaskClass) -> Self {
		Self { class, inner: JoinSet::new() }
	}

	/// Returns the number of tasks currently in the set.
	pub fn len(&self) -> usize {
		self.inner.len()
	}

	/// Returns `true` if the set is empty.
	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	/// Spawns a future into the set on the current worker runtime handle.
	#[allow(clippy::disallowed_methods)]
	pub fn spawn<F>(&mut self, fut: F)
	where
		F: Future<Output = T> + Send + 'static,
	{
		tracing::trace!(worker_class = self.class.as_str(), pending = self.inner.len(), "worker.join_set.spawn");
		let handle = crate::spawn_impl::current_handle();
		let _guard = handle.enter();
		self.inner.spawn(fut);
	}

	/// Waits for the next completed task.
	pub async fn join_next(&mut self) -> Option<Result<T, JoinError>> {
		self.inner.join_next().await
	}

	/// Returns one ready completion without waiting.
	pub fn try_join_next(&mut self) -> Option<Result<T, JoinError>> {
		self.inner.try_join_next()
	}
}
