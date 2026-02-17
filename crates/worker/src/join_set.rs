use std::future::Future;

use tokio::task::{JoinError, JoinSet};

use crate::TaskClass;

/// Join-set wrapper carrying shared worker class metadata and counters.
#[derive(Debug)]
pub struct WorkerJoinSet<T> {
	class: TaskClass,
	inner: JoinSet<T>,
	spawned_total: u64,
	completed_total: u64,
}

impl<T> WorkerJoinSet<T>
where
	T: Send + 'static,
{
	/// Creates a worker join set for one task class.
	pub fn new(class: TaskClass) -> Self {
		Self {
			class,
			inner: JoinSet::new(),
			spawned_total: 0,
			completed_total: 0,
		}
	}

	/// Returns pending task count.
	pub fn len(&self) -> usize {
		self.inner.len()
	}

	/// Returns whether no tasks are pending.
	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	/// Returns total tasks spawned since construction or reset.
	pub fn spawned_total(&self) -> u64 {
		self.spawned_total
	}

	/// Returns total tasks completed (ok or error) since construction or reset.
	pub fn completed_total(&self) -> u64 {
		self.completed_total
	}

	/// Aborts all tasks and resets the join set.
	pub fn abort_all(&mut self) {
		self.inner.abort_all();
		self.inner = JoinSet::new();
	}
	/// Spawns one task into the set.
	pub fn spawn<F>(&mut self, fut: F)
	where
		F: Future<Output = T> + Send + 'static,
	{
		self.spawned_total = self.spawned_total.wrapping_add(1);
		tracing::trace!(worker_class = self.class.as_str(), pending = self.inner.len(), "worker.join_set.spawn");
		self.inner.spawn(fut);
	}

	/// Waits for one completion.
	pub async fn join_next(&mut self) -> Option<Result<T, JoinError>> {
		let res = self.inner.join_next().await;
		if res.is_some() {
			self.completed_total = self.completed_total.wrapping_add(1);
		}
		res
	}
}
