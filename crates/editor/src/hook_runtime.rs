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
use xeno_registry::{BoxFuture as HookBoxFuture, HookPriority, HookScheduler};

/// High-water mark for pending hooks before warning.
const HOOK_BACKLOG_HIGH_WATER: usize = 500;

/// Threshold at which background hooks are dropped to prevent unbounded growth.
const BACKGROUND_DROP_THRESHOLD: usize = 200;

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
	/// Interactive hooks (must complete).
	interactive: FuturesUnordered<HookBoxFuture>,
	/// Background hooks (can be dropped under backlog).
	background: FuturesUnordered<HookBoxFuture>,
	/// Total hooks scheduled (for instrumentation).
	scheduled_total: u64,
	/// Total hooks completed (for instrumentation).
	completed_total: u64,
	/// Background hooks dropped due to backlog.
	dropped_total: u64,
}

impl Default for HookRuntime {
	fn default() -> Self {
		Self {
			interactive: FuturesUnordered::new(),
			background: FuturesUnordered::new(),
			scheduled_total: 0,
			completed_total: 0,
			dropped_total: 0,
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
		!self.interactive.is_empty() || !self.background.is_empty()
	}

	/// Returns the number of pending async hooks.
	pub fn pending_count(&self) -> usize {
		self.interactive.len() + self.background.len()
	}

	/// Returns the number of pending interactive hooks.
	pub fn interactive_count(&self) -> usize {
		self.interactive.len()
	}

	/// Returns the number of pending background hooks.
	pub fn background_count(&self) -> usize {
		self.background.len()
	}

	/// Returns total hooks scheduled since creation.
	pub fn scheduled_total(&self) -> u64 {
		self.scheduled_total
	}

	/// Returns total hooks completed since creation.
	pub fn completed_total(&self) -> u64 {
		self.completed_total
	}

	/// Returns total background hooks dropped due to backlog.
	pub fn dropped_total(&self) -> u64 {
		self.dropped_total
	}

	/// Drains completions within a time budget.
	///
	/// Interactive hooks are processed first (they must complete). Background
	/// hooks are processed only if time remains. Returns promptly when no hooks
	/// are pending, the budget is exhausted, or no hook completes within the
	/// remaining time.
	pub async fn drain_budget(&mut self, budget: Duration) {
		if !self.has_pending() {
			return;
		}

		let start = Instant::now();
		let deadline = start + budget;
		let completed_before = self.completed_total;

		while Instant::now() < deadline && !self.interactive.is_empty() {
			let remaining = deadline.saturating_duration_since(Instant::now());
			match tokio::time::timeout(remaining, self.interactive.next()).await {
				Ok(Some(_)) => {
					self.completed_total += 1;
					tracing::trace!(
						completed_total = self.completed_total,
						interactive_pending = self.interactive.len(),
						priority = "interactive",
						"hook.complete"
					);
				}
				_ => break,
			}
		}

		while Instant::now() < deadline && !self.background.is_empty() {
			let remaining = deadline.saturating_duration_since(Instant::now());
			match tokio::time::timeout(remaining, self.background.next()).await {
				Ok(Some(_)) => {
					self.completed_total += 1;
					tracing::trace!(
						completed_total = self.completed_total,
						background_pending = self.background.len(),
						priority = "background",
						"hook.complete"
					);
				}
				_ => break,
			}
		}

		let completed_this_drain = self.completed_total - completed_before;
		let pending_after = self.pending_count();
		if completed_this_drain > 0 || pending_after > 0 {
			tracing::debug!(
				budget_ms = budget.as_millis() as u64,
				elapsed_ms = start.elapsed().as_millis() as u64,
				completed = completed_this_drain,
				interactive_pending = self.interactive.len(),
				background_pending = self.background.len(),
				"hook.drain_budget"
			);
		}

		if pending_after > HOOK_BACKLOG_HIGH_WATER {
			tracing::warn!(
				interactive_pending = self.interactive.len(),
				background_pending = self.background.len(),
				scheduled = self.scheduled_total,
				completed = self.completed_total,
				dropped = self.dropped_total,
				"hook backlog exceeds high-water mark"
			);
		}
	}

	/// Drains and awaits all queued async hooks concurrently.
	///
	/// Unlike [`drain_budget`](Self::drain_budget), this blocks until all hooks
	/// complete. Use sparingly (e.g., at editor shutdown).
	pub async fn drain_all(&mut self) {
		while let Some(_) = self.interactive.next().await {
			self.completed_total += 1;
		}
		while let Some(_) = self.background.next().await {
			self.completed_total += 1;
		}
	}

	/// Drops all pending background hooks.
	///
	/// Use when the system is under severe load and background work must be shed.
	pub fn drop_background(&mut self) {
		let count = self.background.len();
		if count > 0 {
			self.background = FuturesUnordered::new();
			self.dropped_total += count as u64;
			tracing::info!(dropped = count, "dropped background hooks due to backlog");
		}
	}
}

impl HookScheduler for HookRuntime {
	fn schedule(&mut self, fut: HookBoxFuture, priority: HookPriority) {
		self.scheduled_total += 1;

		match priority {
			HookPriority::Interactive => {
				self.interactive.push(fut);
				tracing::trace!(
					interactive_pending = self.interactive.len(),
					scheduled_total = self.scheduled_total,
					priority = "interactive",
					"hook.schedule"
				);
			}
			HookPriority::Background => {
				if self.background.len() >= BACKGROUND_DROP_THRESHOLD {
					self.dropped_total += 1;
					tracing::debug!(
						background_pending = self.background.len(),
						threshold = BACKGROUND_DROP_THRESHOLD,
						dropped_total = self.dropped_total,
						"dropping background hook due to backlog"
					);
					return;
				}
				self.background.push(fut);
				tracing::trace!(
					background_pending = self.background.len(),
					scheduled_total = self.scheduled_total,
					priority = "background",
					"hook.schedule"
				);
			}
		}
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
		runtime.schedule(
			Box::pin(async move {
				c1.fetch_add(1, Ordering::SeqCst);
				HookResult::Continue
			}),
			HookPriority::Interactive,
		);

		let c2 = counter.clone();
		runtime.schedule(
			Box::pin(async move {
				c2.fetch_add(10, Ordering::SeqCst);
				HookResult::Continue
			}),
			HookPriority::Background,
		);

		assert!(runtime.has_pending());
		assert_eq!(runtime.pending_count(), 2);
		assert_eq!(runtime.interactive_count(), 1);
		assert_eq!(runtime.background_count(), 1);
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

		for _ in 0..2 {
			let c = counter.clone();
			runtime.schedule(
				Box::pin(async move {
					c.fetch_add(1, Ordering::SeqCst);
					HookResult::Continue
				}),
				HookPriority::Interactive,
			);
		}

		assert_eq!(runtime.pending_count(), 2);
		runtime.drain_budget(Duration::from_millis(100)).await;

		assert!(!runtime.has_pending());
		assert_eq!(counter.load(Ordering::SeqCst), 2);
	}

	#[tokio::test]
	async fn test_drain_budget_returns_on_empty() {
		let mut runtime = HookRuntime::new();
		let start = Instant::now();
		runtime.drain_budget(Duration::from_secs(10)).await;
		assert!(start.elapsed() < Duration::from_millis(100));
	}

	#[tokio::test]
	async fn test_drain_budget_respects_timeout() {
		let mut runtime = HookRuntime::new();

		runtime.schedule(
			Box::pin(async {
				tokio::time::sleep(Duration::from_secs(10)).await;
				HookResult::Continue
			}),
			HookPriority::Interactive,
		);

		let start = Instant::now();
		runtime.drain_budget(Duration::from_millis(10)).await;

		assert!(start.elapsed() < Duration::from_millis(100));
		assert!(runtime.has_pending());
	}

	#[tokio::test]
	async fn test_concurrent_execution() {
		let order = Arc::new(std::sync::Mutex::new(Vec::new()));
		let mut runtime = HookRuntime::new();

		for i in 0..3 {
			let o = order.clone();
			runtime.schedule(
				Box::pin(async move {
					o.lock().unwrap().push(i);
					HookResult::Continue
				}),
				HookPriority::Interactive,
			);
		}

		runtime.drain_all().await;

		let completed = order.lock().unwrap();
		assert_eq!(completed.len(), 3);
		assert!(completed.contains(&0));
		assert!(completed.contains(&1));
		assert!(completed.contains(&2));
	}

	#[tokio::test]
	async fn test_interactive_hooks_run_before_background() {
		let order = Arc::new(std::sync::Mutex::new(Vec::new()));
		let mut runtime = HookRuntime::new();

		let o1 = order.clone();
		runtime.schedule(
			Box::pin(async move {
				o1.lock().unwrap().push("background");
				HookResult::Continue
			}),
			HookPriority::Background,
		);

		let o2 = order.clone();
		runtime.schedule(
			Box::pin(async move {
				o2.lock().unwrap().push("interactive");
				HookResult::Continue
			}),
			HookPriority::Interactive,
		);

		runtime.drain_all().await;

		let completed = order.lock().unwrap();
		assert_eq!(completed.len(), 2);
		assert_eq!(completed[0], "interactive");
		assert_eq!(completed[1], "background");
	}

	#[tokio::test]
	async fn test_background_hooks_dropped_under_backlog() {
		let mut runtime = HookRuntime::new();

		for _ in 0..BACKGROUND_DROP_THRESHOLD {
			runtime.schedule(
				Box::pin(async { HookResult::Continue }),
				HookPriority::Background,
			);
		}
		assert_eq!(runtime.background_count(), BACKGROUND_DROP_THRESHOLD);
		assert_eq!(runtime.dropped_total(), 0);

		runtime.schedule(
			Box::pin(async { HookResult::Continue }),
			HookPriority::Background,
		);
		assert_eq!(runtime.background_count(), BACKGROUND_DROP_THRESHOLD);
		assert_eq!(runtime.dropped_total(), 1);

		runtime.schedule(
			Box::pin(async { HookResult::Continue }),
			HookPriority::Interactive,
		);
		assert_eq!(runtime.interactive_count(), 1);
	}

	#[tokio::test]
	async fn test_drop_background() {
		let mut runtime = HookRuntime::new();

		for _ in 0..10 {
			runtime.schedule(
				Box::pin(async { HookResult::Continue }),
				HookPriority::Background,
			);
		}
		runtime.schedule(
			Box::pin(async { HookResult::Continue }),
			HookPriority::Interactive,
		);

		assert_eq!(runtime.background_count(), 10);
		assert_eq!(runtime.interactive_count(), 1);

		runtime.drop_background();

		assert_eq!(runtime.background_count(), 0);
		assert_eq!(runtime.interactive_count(), 1);
		assert_eq!(runtime.dropped_total(), 10);
	}

	#[tokio::test]
	async fn test_drain_budget_does_not_complete_slow_hooks() {
		let mut runtime = HookRuntime::new();

		runtime.schedule(
			Box::pin(async {
				tokio::time::sleep(Duration::from_millis(100)).await;
				HookResult::Continue
			}),
			HookPriority::Interactive,
		);

		runtime.drain_budget(Duration::from_millis(10)).await;
		assert!(runtime.has_pending());
		assert_eq!(runtime.completed_total(), 0);

		runtime.drain_budget(Duration::from_millis(200)).await;
		assert!(!runtime.has_pending());
		assert_eq!(runtime.completed_total(), 1);
	}

	#[tokio::test]
	async fn test_backlog_threshold_exceeded() {
		let mut runtime = HookRuntime::new();

		for _ in 0..HOOK_BACKLOG_HIGH_WATER + 1 {
			runtime.schedule(
				Box::pin(async { HookResult::Continue }),
				HookPriority::Interactive,
			);
		}

		assert!(runtime.pending_count() > HOOK_BACKLOG_HIGH_WATER);
		runtime.drain_all().await;
		assert!(!runtime.has_pending());
	}
}
