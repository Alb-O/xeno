use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{Mutex, Notify};

use crate::actor::{Actor, ActorHandle, ActorRuntime, ActorSpec};
use crate::budget::{DrainBudget, DrainReport};
use crate::join_set::WorkerJoinSet;
use crate::TaskClass;

/// Drop guard that signals a [`Notify`] on completion regardless of exit path
/// (normal return, panic unwind, or future cancellation/abort).
struct NotifyOnDrop(Arc<Notify>);

impl Drop for NotifyOnDrop {
	fn drop(&mut self) {
		self.0.notify_one();
	}
}

fn classify_join_result<T>(class: &str, result: &Result<T, tokio::task::JoinError>, panicked: &mut u64, cancelled: &mut u64) {
	if let Err(err) = result {
		if err.is_panic() {
			*panicked = panicked.wrapping_add(1);
			tracing::warn!(class, "worker.drain: task panicked");
		} else if err.is_cancelled() {
			*cancelled = cancelled.wrapping_add(1);
			tracing::warn!(class, "worker.drain: task cancelled");
		}
	}
}

/// Unified runtime entrypoint for worker task execution and actor supervision.
#[derive(Debug, Clone)]
pub struct WorkerRuntime {
	interactive: Arc<Mutex<WorkerJoinSet<()>>>,
	background: Arc<Mutex<WorkerJoinSet<()>>>,
	interactive_notify: Arc<Notify>,
	background_notify: Arc<Notify>,
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
			interactive: Arc::new(Mutex::new(WorkerJoinSet::new(TaskClass::Interactive))),
			background: Arc::new(Mutex::new(WorkerJoinSet::new(TaskClass::Background))),
			interactive_notify: Arc::new(Notify::new()),
			background_notify: Arc::new(Notify::new()),
		}
	}

	/// Submits managed runtime work drained by [`Self::drain`].
	pub async fn submit<F>(&self, class: TaskClass, fut: F)
	where
		F: Future<Output = ()> + Send + 'static,
	{
		if class == TaskClass::Interactive {
			let notify = Arc::clone(&self.interactive_notify);
			self.interactive.lock().await.spawn(async move {
				let _guard = NotifyOnDrop(notify);
				fut.await;
			});
		} else {
			let notify = Arc::clone(&self.background_notify);
			self.background.lock().await.spawn(async move {
				let _guard = NotifyOnDrop(notify);
				fut.await;
			});
		}
	}

	/// Drains managed runtime work under one budget.
	///
	/// Never holds a join-set mutex across an await point, so concurrent
	/// `submit()` calls are never blocked by an in-progress drain.
	pub(crate) async fn drain(&self, budget: DrainBudget) -> DrainReport {
		if budget.max_completions == 0 {
			let i = self.interactive.lock().await.len();
			let b = self.background.lock().await.len();
			return DrainReport {
				pending_interactive: i,
				pending_background: b,
				..DrainReport::default()
			};
		}

		let deadline = Instant::now() + budget.duration;
		let mut completed = 0u64;
		let mut panicked = 0u64;
		let mut cancelled = 0u64;

		loop {
			if completed as usize >= budget.max_completions || Instant::now() >= deadline {
				break;
			}

			// Register notification futures *before* scanning joinsets to
			// avoid a lost-wakeup: if a task completes between releasing the
			// joinset lock and awaiting the notify, the pre-registered future
			// will still fire.
			let i_notified = self.interactive_notify.notified();
			let b_notified = self.background_notify.notified();
			tokio::pin!(i_notified);
			tokio::pin!(b_notified);
			// Enable the futures so they capture notifications from this point.
			i_notified.as_mut().enable();
			b_notified.as_mut().enable();

			// Fast-path: pop all ready completions without blocking.
			let mut progressed = false;
			let pending_i;
			{
				let mut interactive = self.interactive.lock().await;
				while (completed as usize) < budget.max_completions {
					match interactive.try_join_next() {
						Some(result) => {
							completed = completed.wrapping_add(1);
							progressed = true;
							classify_join_result("interactive", &result, &mut panicked, &mut cancelled);
						}
						None => break,
					}
				}
				pending_i = interactive.len();
			}
			let pending_b;
			{
				let mut background = self.background.lock().await;
				while (completed as usize) < budget.max_completions {
					match background.try_join_next() {
						Some(result) => {
							completed = completed.wrapping_add(1);
							progressed = true;
							classify_join_result("background", &result, &mut panicked, &mut cancelled);
						}
						None => break,
					}
				}
				pending_b = background.len();
			}

			if progressed {
				continue;
			}

			// Both queues empty: nothing to wait for.
			if pending_i == 0 && pending_b == 0 {
				break;
			}

			// No ready completions: wait for a pre-registered notification or deadline.
			let remaining = deadline.saturating_duration_since(Instant::now());
			if remaining.is_zero() {
				break;
			}
			tokio::select! {
				biased;
				_ = i_notified => {}
				_ = b_notified => {}
				_ = tokio::time::sleep(remaining) => { break; }
			}
		}

		let pending_interactive = self.interactive.lock().await.len();
		let pending_background = self.background.lock().await.len();
		DrainReport {
			completed,
			panicked,
			cancelled,
			pending_interactive,
			pending_background,
			budget_exhausted: completed as usize >= budget.max_completions || Instant::now() >= deadline,
		}
	}

	/// Spawns one supervised actor.
	pub fn spawn_actor<A>(&self, spec: ActorSpec<A>) -> ActorHandle<A::Cmd, A::Evt>
	where
		A: Actor,
	{
		ActorRuntime::spawn(spec)
	}

}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
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

	#[tokio::test]
	async fn submit_not_blocked_by_drain() {
		let rt = WorkerRuntime::new();
		let gate = std::sync::Arc::new(tokio::sync::Notify::new());
		let gate_clone = std::sync::Arc::clone(&gate);

		// Submit a long-running task that blocks until we signal it.
		rt.submit(TaskClass::Interactive, async move {
			gate_clone.notified().await;
		})
		.await;

		// Start drain in the background — it will wait for the long task.
		let rt2 = rt.clone();
		let drain_handle = tokio::spawn(async move {
			rt2.drain(DrainBudget {
				duration: Duration::from_millis(500),
				max_completions: 10,
			})
			.await
		});

		// Yield to let drain start waiting.
		tokio::time::sleep(Duration::from_millis(10)).await;

		// submit() must complete quickly even though drain is in progress.
		let submit_ok = tokio::time::timeout(Duration::from_millis(50), rt.submit(TaskClass::Interactive, async {})).await;
		assert!(submit_ok.is_ok(), "submit() should not be blocked by drain()");

		// Release the gate so drain can finish.
		gate.notify_one();
		let _ = drain_handle.await;
	}

	#[tokio::test]
	async fn drain_returns_immediately_when_empty() {
		let rt = WorkerRuntime::new();
		let report = tokio::time::timeout(
			Duration::from_millis(50),
			rt.drain(DrainBudget {
				duration: Duration::from_secs(5),
				max_completions: 1,
			}),
		)
		.await
		.expect("drain() should return immediately when no tasks are pending");
		assert_eq!(report.completed, 0);
		assert_eq!(report.pending_interactive, 0);
		assert_eq!(report.pending_background, 0);
		assert!(!report.budget_exhausted);
	}

	#[tokio::test]
	async fn drain_wakes_on_panicking_task_completion() {
		let rt = WorkerRuntime::new();
		let gate = Arc::new(tokio::sync::Notify::new());
		let gate2 = Arc::clone(&gate);

		rt.submit(TaskClass::Interactive, async move {
			gate2.notified().await;
			panic!("boom");
		})
		.await;

		let rt2 = rt.clone();
		let drain_task = tokio::spawn(async move {
			rt2.drain(DrainBudget {
				duration: Duration::from_millis(200),
				max_completions: 1,
			})
			.await
		});

		tokio::task::yield_now().await;
		gate.notify_one();

		let report = drain_task.await.unwrap();
		assert_eq!(report.completed, 1, "drain should count panicked task as completed");
		assert_eq!(report.panicked, 1, "drain should report the panic");
		assert_eq!(report.cancelled, 0);
	}
}
