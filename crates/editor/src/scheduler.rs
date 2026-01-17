//! Unified async work scheduler with backpressure and cancellation.
//!
//! This module provides [`WorkScheduler`], a single backpressure system for
//! async work including hooks, LSP flushes, indexing, and file watchers.
//!
//! # Design
//!
//! Work items are categorized by kind and priority:
//! - **Interactive** work must complete (LSP sync, user-visible feedback)
//! - **Background** work can be dropped under backlog (analytics, telemetry)
//!
//! Cancellation is supported by `(doc_id, kind)` for document-specific work.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use xeno_registry::HookPriority;

/// Unique identifier for a document (used for cancellation).
pub type DocId = u64;

/// Kind of async work being scheduled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkKind {
	/// Hook execution.
	Hook,
	/// LSP document sync flush.
	LspFlush,
	/// File indexing.
	Indexing,
	/// File watcher event processing.
	Watcher,
}

/// A scheduled work item.
pub struct WorkItem {
	/// The future to execute.
	pub future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
	/// Kind of work.
	pub kind: WorkKind,
	/// Execution priority.
	pub priority: HookPriority,
	/// Optional document ID for cancellation.
	pub doc_id: Option<DocId>,
}

/// Unified scheduler for async work.
///
/// Routes all async work (hooks, LSP, indexing, watchers) through a single
/// backpressure system with explicit budgets and priorities.
pub struct WorkScheduler {
	/// Interactive work (must complete).
	interactive: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
	/// Background work (can be dropped).
	background: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
	/// Pending work items by (doc_id, kind) for cancellation.
	pending_by_doc: HashMap<(DocId, WorkKind), usize>,
	/// Total work scheduled.
	scheduled_total: u64,
	/// Total work completed.
	completed_total: u64,
	/// Background work dropped due to backlog.
	dropped_total: u64,
}

/// High-water mark for total pending work before warning.
const BACKLOG_HIGH_WATER: usize = 500;

/// Threshold at which background work is dropped.
const BACKGROUND_DROP_THRESHOLD: usize = 200;

impl Default for WorkScheduler {
	fn default() -> Self {
		Self {
			interactive: FuturesUnordered::new(),
			background: FuturesUnordered::new(),
			pending_by_doc: HashMap::new(),
			scheduled_total: 0,
			completed_total: 0,
			dropped_total: 0,
		}
	}
}

impl WorkScheduler {
	/// Creates a new scheduler.
	pub fn new() -> Self {
		Self::default()
	}

	/// Schedules a work item.
	pub fn schedule(&mut self, item: WorkItem) {
		self.scheduled_total += 1;

		if let Some(doc_id) = item.doc_id {
			*self.pending_by_doc.entry((doc_id, item.kind)).or_insert(0) += 1;
		}

		match item.priority {
			HookPriority::Interactive => {
				self.interactive.push(item.future);
				tracing::trace!(
					interactive_pending = self.interactive.len(),
					kind = ?item.kind,
					scheduled_total = self.scheduled_total,
					"work.schedule"
				);
			}
			HookPriority::Background => {
				if self.background.len() >= BACKGROUND_DROP_THRESHOLD {
					self.dropped_total += 1;
					if let Some(doc_id) = item.doc_id
						&& let Some(count) = self.pending_by_doc.get_mut(&(doc_id, item.kind))
					{
						*count = count.saturating_sub(1);
					}
					tracing::debug!(
						background_pending = self.background.len(),
						kind = ?item.kind,
						dropped_total = self.dropped_total,
						"dropping background work due to backlog"
					);
					return;
				}
				self.background.push(item.future);
				tracing::trace!(
					background_pending = self.background.len(),
					kind = ?item.kind,
					scheduled_total = self.scheduled_total,
					"work.schedule"
				);
			}
		}
	}

	/// Cancels pending work for a specific document and kind.
	///
	/// Returns the number of items that were marked for cancellation.
	/// Note: Already-running futures cannot be cancelled; this prevents
	/// new scheduling and clears pending counts.
	pub fn cancel(&mut self, doc_id: DocId, kind: WorkKind) -> usize {
		let count = self.pending_by_doc.remove(&(doc_id, kind)).unwrap_or(0);
		if count > 0 {
			tracing::debug!(doc_id, kind = ?kind, count, "work.cancel");
		}
		count
	}

	/// Returns the count of pending work for a document and kind.
	pub fn pending_for_doc(&self, doc_id: DocId, kind: WorkKind) -> usize {
		self.pending_by_doc
			.get(&(doc_id, kind))
			.copied()
			.unwrap_or(0)
	}

	/// Returns true if there is pending work.
	pub fn has_pending(&self) -> bool {
		!self.interactive.is_empty() || !self.background.is_empty()
	}

	/// Returns total pending work count.
	pub fn pending_count(&self) -> usize {
		self.interactive.len() + self.background.len()
	}

	/// Returns pending interactive work count.
	pub fn interactive_count(&self) -> usize {
		self.interactive.len()
	}

	/// Returns pending background work count.
	pub fn background_count(&self) -> usize {
		self.background.len()
	}

	/// Returns total work scheduled.
	pub fn scheduled_total(&self) -> u64 {
		self.scheduled_total
	}

	/// Returns total work completed.
	pub fn completed_total(&self) -> u64 {
		self.completed_total
	}

	/// Returns total background work dropped.
	pub fn dropped_total(&self) -> u64 {
		self.dropped_total
	}

	/// Drains completions within a time budget.
	///
	/// Interactive work is processed first, then background if time remains.
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
				Ok(Some(())) => {
					self.completed_total += 1;
				}
				_ => break,
			}
		}

		while Instant::now() < deadline && !self.background.is_empty() {
			let remaining = deadline.saturating_duration_since(Instant::now());
			match tokio::time::timeout(remaining, self.background.next()).await {
				Ok(Some(())) => {
					self.completed_total += 1;
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
				"work.drain_budget"
			);
		}

		if pending_after > BACKLOG_HIGH_WATER {
			tracing::warn!(
				interactive_pending = self.interactive.len(),
				background_pending = self.background.len(),
				scheduled = self.scheduled_total,
				completed = self.completed_total,
				dropped = self.dropped_total,
				"work backlog exceeds high-water mark"
			);
		}
	}

	/// Drains all pending work.
	pub async fn drain_all(&mut self) {
		while let Some(()) = self.interactive.next().await {
			self.completed_total += 1;
		}
		while let Some(()) = self.background.next().await {
			self.completed_total += 1;
		}
	}

	/// Drops all pending background work.
	pub fn drop_background(&mut self) {
		let count = self.background.len();
		if count > 0 {
			self.background = FuturesUnordered::new();
			self.dropped_total += count as u64;
			tracing::info!(dropped = count, "dropped all background work");
		}
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	use super::*;

	#[tokio::test]
	async fn test_schedule_and_drain() {
		let counter = Arc::new(AtomicUsize::new(0));
		let mut scheduler = WorkScheduler::new();

		let c = counter.clone();
		scheduler.schedule(WorkItem {
			future: Box::pin(async move {
				c.fetch_add(1, Ordering::SeqCst);
			}),
			kind: WorkKind::Hook,
			priority: HookPriority::Interactive,
			doc_id: None,
		});

		let c = counter.clone();
		scheduler.schedule(WorkItem {
			future: Box::pin(async move {
				c.fetch_add(10, Ordering::SeqCst);
			}),
			kind: WorkKind::LspFlush,
			priority: HookPriority::Background,
			doc_id: Some(1),
		});

		assert_eq!(scheduler.pending_count(), 2);
		scheduler.drain_all().await;
		assert_eq!(counter.load(Ordering::SeqCst), 11);
	}

	#[tokio::test]
	async fn test_interactive_before_background() {
		let order = Arc::new(std::sync::Mutex::new(Vec::new()));
		let mut scheduler = WorkScheduler::new();

		let o = order.clone();
		scheduler.schedule(WorkItem {
			future: Box::pin(async move {
				o.lock().unwrap().push("background");
			}),
			kind: WorkKind::Hook,
			priority: HookPriority::Background,
			doc_id: None,
		});

		let o = order.clone();
		scheduler.schedule(WorkItem {
			future: Box::pin(async move {
				o.lock().unwrap().push("interactive");
			}),
			kind: WorkKind::Hook,
			priority: HookPriority::Interactive,
			doc_id: None,
		});

		scheduler.drain_all().await;
		let completed = order.lock().unwrap();
		assert_eq!(completed[0], "interactive");
		assert_eq!(completed[1], "background");
	}

	#[tokio::test]
	async fn test_background_drop_under_backlog() {
		let mut scheduler = WorkScheduler::new();

		for _ in 0..BACKGROUND_DROP_THRESHOLD {
			scheduler.schedule(WorkItem {
				future: Box::pin(async {}),
				kind: WorkKind::Hook,
				priority: HookPriority::Background,
				doc_id: None,
			});
		}
		assert_eq!(scheduler.dropped_total(), 0);

		scheduler.schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::Hook,
			priority: HookPriority::Background,
			doc_id: None,
		});
		assert_eq!(scheduler.dropped_total(), 1);
	}

	#[tokio::test]
	async fn test_pending_for_doc() {
		let mut scheduler = WorkScheduler::new();

		scheduler.schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::LspFlush,
			priority: HookPriority::Interactive,
			doc_id: Some(42),
		});

		scheduler.schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::LspFlush,
			priority: HookPriority::Interactive,
			doc_id: Some(42),
		});

		scheduler.schedule(WorkItem {
			future: Box::pin(async {}),
			kind: WorkKind::Hook,
			priority: HookPriority::Interactive,
			doc_id: Some(42),
		});

		assert_eq!(scheduler.pending_for_doc(42, WorkKind::LspFlush), 2);
		assert_eq!(scheduler.pending_for_doc(42, WorkKind::Hook), 1);
		assert_eq!(scheduler.pending_for_doc(99, WorkKind::LspFlush), 0);
	}

	#[tokio::test]
	async fn test_cancel() {
		let mut scheduler = WorkScheduler::new();

		for _ in 0..3 {
			scheduler.schedule(WorkItem {
				future: Box::pin(async {}),
				kind: WorkKind::LspFlush,
				priority: HookPriority::Interactive,
				doc_id: Some(42),
			});
		}

		assert_eq!(scheduler.pending_for_doc(42, WorkKind::LspFlush), 3);
		let cancelled = scheduler.cancel(42, WorkKind::LspFlush);
		assert_eq!(cancelled, 3);
		assert_eq!(scheduler.pending_for_doc(42, WorkKind::LspFlush), 0);
	}
}
