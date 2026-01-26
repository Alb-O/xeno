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

#[derive(Debug, Clone, Copy)]
pub struct HookDrainBudget {
	pub duration: Duration,
	pub max_completions: usize,
}

impl HookDrainBudget {
	pub fn new(duration: Duration, max_completions: usize) -> Self {
		Self {
			duration,
			max_completions,
		}
	}
}

impl From<Duration> for HookDrainBudget {
	fn from(duration: Duration) -> Self {
		Self {
			duration,
			max_completions: usize::MAX,
		}
	}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HookDrainStats {
	pub completed: u64,
	pub pending: usize,
}

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

	/// Drains completions within a time and count budget.
	///
	/// Interactive hooks are processed first (they must complete). Background
	/// hooks are processed only if time remains. Returns promptly when no hooks
	/// are pending, the budget is exhausted, or no hook completes within the
	/// remaining time.
	pub async fn drain_budget(&mut self, budget: impl Into<HookDrainBudget>) -> HookDrainStats {
		if !self.has_pending() {
			return HookDrainStats::default();
		}

		let budget = budget.into();
		let start = Instant::now();
		let deadline = start + budget.duration;
		let completed_before = self.completed_total;
		let mut remaining = budget.max_completions;

		if remaining == 0 {
			return HookDrainStats {
				completed: 0,
				pending: self.pending_count(),
			};
		}

		while Instant::now() < deadline && !self.interactive.is_empty() && remaining > 0 {
			let time_left = deadline.saturating_duration_since(Instant::now());
			match tokio::time::timeout(time_left, self.interactive.next()).await {
				Ok(Some(_)) => {
					self.completed_total += 1;
					remaining = remaining.saturating_sub(1);
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

		while Instant::now() < deadline && !self.background.is_empty() && remaining > 0 {
			let time_left = deadline.saturating_duration_since(Instant::now());
			match tokio::time::timeout(time_left, self.background.next()).await {
				Ok(Some(_)) => {
					self.completed_total += 1;
					remaining = remaining.saturating_sub(1);
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
				budget_ms = budget.duration.as_millis() as u64,
				elapsed_ms = start.elapsed().as_millis() as u64,
				completed = completed_this_drain,
				budget_max = budget.max_completions,
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

		HookDrainStats {
			completed: completed_this_drain,
			pending: pending_after,
		}
	}

	/// Drains and awaits all queued async hooks concurrently.
	///
	/// Unlike [`drain_budget`](Self::drain_budget), this blocks until all hooks
	/// complete. Use sparingly (e.g., at editor shutdown).
	pub async fn drain_all(&mut self) {
		while self.interactive.next().await.is_some() {
			self.completed_total += 1;
		}
		while self.background.next().await.is_some() {
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
mod tests;
