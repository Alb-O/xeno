use std::future::Future;
use std::time::Instant;

use tokio::sync::Mutex;

use crate::budget::{DrainBudget, DrainReport};
use crate::join_set::WorkerJoinSet;
use crate::registry::WorkerRegistry;
use crate::supervisor::{ActorHandle, ActorSpec, WorkerActor, spawn_supervised_actor};
use crate::{TaskClass, spawn, spawn_blocking, spawn_named_thread, spawn_thread};

/// Unified runtime entrypoint for worker task execution and actor supervision.
#[derive(Debug, Clone)]
pub struct WorkerRuntime {
	interactive: std::sync::Arc<Mutex<WorkerJoinSet<()>>>,
	background: std::sync::Arc<Mutex<WorkerJoinSet<()>>>,
	registry: WorkerRegistry,
}

impl Default for WorkerRuntime {
	fn default() -> Self {
		Self::new()
	}
}

impl WorkerRuntime {
	/// Creates a runtime with empty managed queues.
	pub fn new() -> Self {
		Self {
			interactive: std::sync::Arc::new(Mutex::new(WorkerJoinSet::new(TaskClass::Interactive))),
			background: std::sync::Arc::new(Mutex::new(WorkerJoinSet::new(TaskClass::Background))),
			registry: WorkerRegistry::new(),
		}
	}

	/// Spawns an async task.
	pub fn spawn<F>(&self, class: TaskClass, fut: F) -> tokio::task::JoinHandle<F::Output>
	where
		F: Future + Send + 'static,
		F::Output: Send + 'static,
	{
		spawn(class, fut)
	}

	/// Spawns blocking work.
	pub fn spawn_blocking<F, R>(&self, class: TaskClass, f: F) -> tokio::task::JoinHandle<R>
	where
		F: FnOnce() -> R + Send + 'static,
		R: Send + 'static,
	{
		spawn_blocking(class, f)
	}

	/// Spawns an OS thread.
	pub fn spawn_thread<F, R>(&self, class: TaskClass, f: F) -> std::thread::JoinHandle<R>
	where
		F: FnOnce() -> R + Send + 'static,
		R: Send + 'static,
	{
		spawn_thread(class, f)
	}

	/// Spawns a named OS thread.
	pub fn spawn_named_thread<F, R>(&self, class: TaskClass, name: impl Into<String>, f: F) -> std::io::Result<std::thread::JoinHandle<R>>
	where
		F: FnOnce() -> R + Send + 'static,
		R: Send + 'static,
	{
		spawn_named_thread(class, name, f)
	}

	/// Submits managed runtime work drained by [`Self::drain`].
	pub async fn submit<F>(&self, class: TaskClass, fut: F)
	where
		F: Future<Output = ()> + Send + 'static,
	{
		if class == TaskClass::Interactive {
			self.interactive.lock().await.spawn(fut);
		} else {
			self.background.lock().await.spawn(fut);
		}
	}

	/// Drains managed runtime work under one budget.
	pub async fn drain(&self, budget: DrainBudget) -> DrainReport {
		if budget.max_completions == 0 {
			let i = self.interactive.lock().await.len();
			let b = self.background.lock().await.len();
			return DrainReport {
				pending_interactive: i,
				pending_background: b,
				..DrainReport::default()
			};
		}

		let start = Instant::now();
		let deadline = start + budget.duration;
		let mut completed = 0u64;

		loop {
			if completed as usize >= budget.max_completions || Instant::now() >= deadline {
				break;
			}

			let did_work = {
				let mut interactive = self.interactive.lock().await;
				if interactive.is_empty() {
					false
				} else {
					match tokio::time::timeout(deadline.saturating_duration_since(Instant::now()), interactive.join_next()).await {
						Ok(Some(_)) => {
							completed = completed.wrapping_add(1);
							true
						}
						_ => false,
					}
				}
			};
			if did_work {
				continue;
			}

			let did_bg = {
				let mut background = self.background.lock().await;
				if background.is_empty() {
					false
				} else {
					match tokio::time::timeout(deadline.saturating_duration_since(Instant::now()), background.join_next()).await {
						Ok(Some(_)) => {
							completed = completed.wrapping_add(1);
							true
						}
						_ => false,
					}
				}
			};

			if !did_bg {
				break;
			}
		}

		let pending_interactive = self.interactive.lock().await.len();
		let pending_background = self.background.lock().await.len();
		DrainReport {
			completed,
			pending_interactive,
			pending_background,
			budget_exhausted: completed as usize >= budget.max_completions || Instant::now() >= deadline,
		}
	}

	/// Spawns one supervised actor.
	pub fn actor<A>(&self, spec: ActorSpec<A>) -> ActorHandle<A::Cmd, A::Evt>
	where
		A: WorkerActor,
	{
		spawn_supervised_actor(spec)
	}

	/// Returns the shared worker registry.
	pub fn registry(&self) -> &WorkerRegistry {
		&self.registry
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use super::*;

	#[tokio::test]
	async fn managed_submit_and_drain() {
		let rt = WorkerRuntime::new();
		for _ in 0..5 {
			rt.submit(TaskClass::Interactive, async {}).await;
		}

		let report = rt
			.drain(DrainBudget {
				duration: Duration::from_secs(1),
				max_completions: 3,
			})
			.await;
		assert_eq!(report.completed, 3);
		assert_eq!(report.pending_interactive, 2);
	}
}
