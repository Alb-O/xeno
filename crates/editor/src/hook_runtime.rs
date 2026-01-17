//! Hook runtime for scheduling and executing async hooks.
//!
//! This module provides [`HookRuntime`], which stores queued async hook futures
//! and provides methods to drain them. It integrates with the sync emission path
//! via [`HookScheduler`].
//!
//! # Concurrency Model
//!
//! Hooks execute concurrently via [`FuturesUnordered`], not sequentially. The
//! [`drain_budget`](HookRuntime::drain_budget) method polls completions within a
//! time budget, preventing UI stalls from slow hooks.

use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use xeno_registry::{BoxFuture as HookBoxFuture, HookScheduler};

/// High-water mark for pending hooks before warning.
const HOOK_BACKLOG_HIGH_WATER: usize = 500;

/// Runtime for managing async hook execution.
///
/// Async hooks queued during sync emission are stored here and drained
/// by the main loop (typically once per tick or after each event batch).
///
/// # Example
///
/// ```ignore
/// let mut runtime = HookRuntime::new();
///
/// // During sync event handling:
/// emit_hook_sync_with(&HookContext::new(HookEventData::EditorTick, None), &mut runtime);
///
/// // Later, drain queued async work with a time budget:
/// runtime.drain_budget(Duration::from_millis(2)).await;
/// ```
pub struct HookRuntime {
	/// Concurrent set of pending async hook futures.
	running: FuturesUnordered<HookBoxFuture>,
	/// Total hooks scheduled (for instrumentation).
	scheduled_total: u64,
	/// Total hooks completed (for instrumentation).
	completed_total: u64,
}

impl Default for HookRuntime {
	fn default() -> Self {
		Self {
			running: FuturesUnordered::new(),
			scheduled_total: 0,
			completed_total: 0,
		}
	}
}

impl HookRuntime {
	/// Creates a new empty hook runtime.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns true if there are pending async hooks.
	pub fn has_pending(&self) -> bool {
		!self.running.is_empty()
	}

	/// Returns the number of pending async hooks.
	pub fn pending_count(&self) -> usize {
		self.running.len()
	}

	/// Returns total hooks scheduled since creation.
	pub fn scheduled_total(&self) -> u64 {
		self.scheduled_total
	}

	/// Returns total hooks completed since creation.
	pub fn completed_total(&self) -> u64 {
		self.completed_total
	}

	/// Drains completions within a time budget.
	///
	/// Returns promptly when no hooks are pending, the budget is exhausted,
	/// or no hook completes within the remaining time. This keeps the main
	/// loop responsive even when hooks are slow.
	pub async fn drain_budget(&mut self, budget: Duration) {
		if self.running.is_empty() {
			return;
		}

		let deadline = Instant::now() + budget;
		while Instant::now() < deadline {
			let remaining = deadline.saturating_duration_since(Instant::now());
			match tokio::time::timeout(remaining, self.running.next()).await {
				Ok(Some(_)) => self.completed_total += 1,
				_ => break,
			}
		}

		if self.running.len() > HOOK_BACKLOG_HIGH_WATER {
			tracing::warn!(
				pending = self.running.len(),
				scheduled = self.scheduled_total,
				completed = self.completed_total,
				"hook backlog exceeds high-water mark"
			);
		}
	}

	/// Drains and awaits all queued async hooks concurrently.
	///
	/// Unlike [`drain_budget`](Self::drain_budget), this blocks until all hooks
	/// complete. Use sparingly (e.g., at editor shutdown).
	pub async fn drain_all(&mut self) {
		while let Some(_) = self.running.next().await {
			self.completed_total += 1;
		}
	}
}

impl HookScheduler for HookRuntime {
	fn schedule(&mut self, fut: HookBoxFuture) {
		self.running.push(fut);
		self.scheduled_total += 1;
	}
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::{AtomicUsize, Ordering};
	use std::sync::Arc;

	use xeno_registry::HookResult;

	use super::*;

	#[tokio::test]
	async fn test_empty_drain() {
		let mut runtime = HookRuntime::new();
		assert!(!runtime.has_pending());
		runtime.drain_all().await;
		assert!(!runtime.has_pending());
	}

	#[tokio::test]
	async fn test_schedule_and_drain_all() {
		let counter = Arc::new(AtomicUsize::new(0));
		let mut runtime = HookRuntime::new();

		let c1 = counter.clone();
		runtime.schedule(Box::pin(async move {
			c1.fetch_add(1, Ordering::SeqCst);
			HookResult::Continue
		}));

		let c2 = counter.clone();
		runtime.schedule(Box::pin(async move {
			c2.fetch_add(10, Ordering::SeqCst);
			HookResult::Continue
		}));

		assert!(runtime.has_pending());
		assert_eq!(runtime.pending_count(), 2);
		assert_eq!(runtime.scheduled_total(), 2);
		assert_eq!(runtime.completed_total(), 0);

		runtime.drain_all().await;

		assert!(!runtime.has_pending());
		assert_eq!(counter.load(Ordering::SeqCst), 11);
		assert_eq!(runtime.completed_total(), 2);
	}

	#[tokio::test]
	async fn test_drain_budget_completes_fast_hooks() {
		let counter = Arc::new(AtomicUsize::new(0));
		let mut runtime = HookRuntime::new();

		// Schedule two fast hooks.
		for _ in 0..2 {
			let c = counter.clone();
			runtime.schedule(Box::pin(async move {
				c.fetch_add(1, Ordering::SeqCst);
				HookResult::Continue
			}));
		}

		assert_eq!(runtime.pending_count(), 2);

		// Generous budget should complete all fast hooks.
		runtime.drain_budget(Duration::from_millis(100)).await;

		assert!(!runtime.has_pending());
		assert_eq!(counter.load(Ordering::SeqCst), 2);
	}

	#[tokio::test]
	async fn test_drain_budget_returns_on_empty() {
		let mut runtime = HookRuntime::new();
		let start = Instant::now();
		runtime.drain_budget(Duration::from_secs(10)).await;
		// Should return immediately when empty, not wait 10 seconds.
		assert!(start.elapsed() < Duration::from_millis(100));
	}

	#[tokio::test]
	async fn test_drain_budget_respects_timeout() {
		let mut runtime = HookRuntime::new();

		// Schedule a hook that takes a long time.
		runtime.schedule(Box::pin(async {
			tokio::time::sleep(Duration::from_secs(10)).await;
			HookResult::Continue
		}));

		let start = Instant::now();
		// Use a short budget.
		runtime.drain_budget(Duration::from_millis(10)).await;

		// Should return near the budget, not wait for the slow hook.
		assert!(start.elapsed() < Duration::from_millis(100));
		// Hook is still pending.
		assert!(runtime.has_pending());
	}

	#[tokio::test]
	async fn test_concurrent_execution() {
		let order = Arc::new(std::sync::Mutex::new(Vec::new()));
		let mut runtime = HookRuntime::new();

		// Schedule hooks that record completion order.
		// With concurrent execution, order may vary.
		for i in 0..3 {
			let o = order.clone();
			runtime.schedule(Box::pin(async move {
				o.lock().unwrap().push(i);
				HookResult::Continue
			}));
		}

		runtime.drain_all().await;

		let completed = order.lock().unwrap();
		assert_eq!(completed.len(), 3);
		// All hooks completed (order may vary due to concurrency).
		assert!(completed.contains(&0));
		assert!(completed.contains(&1));
		assert!(completed.contains(&2));
	}
}
