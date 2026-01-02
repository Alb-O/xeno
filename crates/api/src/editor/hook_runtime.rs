//! Hook runtime for scheduling and executing async hooks.
//!
//! This module provides [`HookRuntime`], which stores queued async hook futures
//! and provides methods to drain them. It integrates with the sync emission path
//! via [`HookScheduler`].

use std::collections::VecDeque;

use evildoer_registry::{BoxFuture as HookBoxFuture, HookScheduler};

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
/// // Later, drain queued async work:
/// runtime.drain().await;
/// ```
#[derive(Default)]
pub struct HookRuntime {
	/// FIFO queue of pending async hook futures.
	queue: VecDeque<HookBoxFuture>,
}

impl HookRuntime {
	/// Creates a new empty hook runtime.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns true if there are pending async hooks.
	pub fn has_pending(&self) -> bool {
		!self.queue.is_empty()
	}

	/// Returns the number of pending async hooks.
	pub fn pending_count(&self) -> usize {
		self.queue.len()
	}

	/// Drains and awaits all queued async hooks in FIFO order.
	pub async fn drain(&mut self) {
		while let Some(fut) = self.queue.pop_front() {
			let _ = fut.await;
		}
	}

	/// Takes all pending futures without awaiting them.
	///
	/// Useful for transferring ownership or testing.
	pub fn take_all(&mut self) -> VecDeque<HookBoxFuture> {
		std::mem::take(&mut self.queue)
	}
}

impl HookScheduler for HookRuntime {
	fn schedule(&mut self, fut: HookBoxFuture) {
		self.queue.push_back(fut);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_empty_drain() {
		let mut runtime = HookRuntime::new();
		assert!(!runtime.has_pending());
		runtime.drain().await;
		assert!(!runtime.has_pending());
	}

	#[tokio::test]
	async fn test_schedule_and_drain() {
		use std::sync::Arc;
		use std::sync::atomic::{AtomicUsize, Ordering};

		use evildoer_registry::HookResult;

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

		runtime.drain().await;

		assert!(!runtime.has_pending());
		assert_eq!(counter.load(Ordering::SeqCst), 11);
	}
}
