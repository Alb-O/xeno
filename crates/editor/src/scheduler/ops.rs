use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use xeno_registry::hooks::{HookFuture as HookBoxFuture, HookPriority, HookScheduler};

use super::state::{BACKGROUND_DROP_THRESHOLD, BACKLOG_HIGH_WATER, CANCELLED_DOC_LRU_CAP, WorkScheduler};
use super::types::{DocId, WorkItem, WorkKind};

/// RAII guard that fires [`Notify::notify_one`] on drop, ensuring drain
/// wakes up even if the wrapped future panics or is cancelled.
struct NotifyOnDrop(Arc<Notify>);
impl Drop for NotifyOnDrop {
	fn drop(&mut self) {
		self.0.notify_one();
	}
}

/// RAII guard that decrements a pending-by-doc counter on drop.
///
/// Ensures the counter stays accurate through normal completion, panic
/// unwind, and cancellation.
struct PendingCountGuard(Arc<AtomicUsize>);
impl Drop for PendingCountGuard {
	fn drop(&mut self) {
		let _ = self.0.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(v.saturating_sub(1)));
	}
}

/// Runs a future with optional doc-level and kind-level cancellation.
async fn schedule_inner(
	doc_token: Option<CancellationToken>,
	kind_token: Option<CancellationToken>,
	future: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>,
) {
	match (doc_token, kind_token) {
		(Some(doc_tok), Some(kind_tok)) => {
			tokio::select! {
				biased;
				_ = doc_tok.cancelled() => {}
				_ = kind_tok.cancelled() => {}
				_ = future => {}
			}
		}
		_ => future.await,
	}
}

impl WorkScheduler {
	/// Creates a new scheduler.
	pub fn new() -> Self {
		Self::default()
	}

	/// Schedules a work item.
	///
	/// If the item has a `doc_id` whose cancellation token is already fired
	/// (via `cancel_doc()`), the item is dropped without spawning.
	pub fn schedule(&mut self, item: WorkItem) {
		self.scheduled_total += 1;

		// Check cancelled-docs LRU — short-circuit without spawning or creating tokens.
		if let Some(doc_id) = item.doc_id
			&& self.cancelled_docs.contains(&doc_id)
		{
			tracing::trace!(
				doc_id,
				kind = ?item.kind,
				"work.schedule: skipped (doc cancelled)"
			);
			return;
		}

		// Background backlog check — before creating tokens or guards.
		if item.priority == HookPriority::Background && self.background.len() >= BACKGROUND_DROP_THRESHOLD {
			self.dropped_total += 1;
			tracing::debug!(
				background_pending = self.background.len(),
				kind = ?item.kind,
				dropped_total = self.dropped_total,
				"dropping background work due to backlog"
			);
			return;
		}

		// Build cancellation tokens and pending-count guard for doc-scoped items.
		let doc_token = item.doc_id.map(|id| self.doc_token(id));
		let kind_token = item.doc_id.map(|id| self.kind_token(id, item.kind));
		let pending_guard = item.doc_id.map(|doc_id| {
			let counter = self.pending_by_doc.entry((doc_id, item.kind)).or_insert_with(|| Arc::new(AtomicUsize::new(0)));
			counter.fetch_add(1, Ordering::Relaxed);
			PendingCountGuard(Arc::clone(counter))
		});

		match item.priority {
			HookPriority::Interactive => {
				let guard = self.gate.enter_interactive();
				let notify = Arc::clone(&self.interactive_notify);
				self.interactive.spawn(async move {
					let _notify = NotifyOnDrop(notify);
					let _guard = guard;
					let _pending = pending_guard;
					schedule_inner(doc_token, kind_token, item.future).await;
				});
				tracing::trace!(
					interactive_pending = self.interactive.len(),
					kind = ?item.kind,
					scheduled_total = self.scheduled_total,
					"work.schedule"
				);
			}
			HookPriority::Background => {
				let gate = self.gate.clone();
				let notify = Arc::clone(&self.background_notify);
				self.background.spawn(async move {
					let _notify = NotifyOnDrop(notify);
					let _pending = pending_guard;
					gate.wait_for_background().await;
					schedule_inner(doc_token, kind_token, item.future).await;
				});
				tracing::trace!(
					background_pending = self.background.len(),
					kind = ?item.kind,
					scheduled_total = self.scheduled_total,
					"work.schedule"
				);
			}
		}
	}

	/// Returns or creates a per-(doc, kind) cancellation token.
	fn kind_token(&mut self, doc_id: DocId, kind: WorkKind) -> CancellationToken {
		self.kind_cancel.entry((doc_id, kind)).or_default().clone()
	}

	/// Cancels pending work for a specific document and kind.
	///
	/// Fires the per-(doc, kind) cancellation token so in-flight futures
	/// exit early, and removes the pending counter entry.
	#[cfg(test)]
	pub fn cancel(&mut self, doc_id: DocId, kind: WorkKind) -> usize {
		let key = (doc_id, kind);
		if let Some(tok) = self.kind_cancel.remove(&key) {
			tok.cancel();
		}
		let count = self.pending_by_doc.remove(&key).map(|c| c.load(Ordering::Relaxed)).unwrap_or(0);
		if count > 0 {
			tracing::debug!(doc_id, kind = ?kind, count, "work.cancel");
		}
		count
	}

	/// Returns or creates a cancellation token for a document.
	///
	/// Scheduled futures for this doc_id will be cancelled when `cancel_doc()`
	/// is called (typically on buffer close).
	fn doc_token(&mut self, doc_id: DocId) -> CancellationToken {
		self.doc_cancel.entry(doc_id).or_default().clone()
	}

	/// Cancels all scheduled work for a document by firing its cancellation token.
	///
	/// The doc id is added to a bounded LRU set so that future `schedule()`
	/// calls for this doc_id are short-circuited without creating tokens.
	/// Also purges `pending_by_doc` bookkeeping for the document.
	pub fn cancel_doc(&mut self, doc_id: DocId) {
		if let Some(token) = self.doc_cancel.remove(&doc_id) {
			token.cancel();
		}
		self.kind_cancel.retain(|(id, _), tok| {
			if *id == doc_id {
				tok.cancel();
				false
			} else {
				true
			}
		});
		self.mark_doc_cancelled(doc_id);
		self.pending_by_doc.retain(|(id, _), _| *id != doc_id);
		tracing::debug!(doc_id, "work.cancel_doc");
	}

	/// Adds a doc id to the bounded cancelled-docs LRU set.
	fn mark_doc_cancelled(&mut self, doc_id: DocId) {
		if self.cancelled_docs.insert(doc_id) {
			self.cancelled_docs_order.push_back(doc_id);
		}
		while self.cancelled_docs_order.len() > CANCELLED_DOC_LRU_CAP {
			if let Some(evicted) = self.cancelled_docs_order.pop_front() {
				self.cancelled_docs.remove(&evicted);
			}
		}
	}

	/// Returns the number of active cancellation tokens (test helper).
	#[cfg(test)]
	pub(super) fn doc_cancel_len(&self) -> usize {
		self.doc_cancel.len()
	}

	/// Returns the number of entries in the cancelled-docs LRU (test helper).
	#[cfg(test)]
	pub(super) fn cancelled_docs_len(&self) -> usize {
		self.cancelled_docs.len()
	}

	/// Returns the count of pending work for a document and kind.
	#[cfg(test)]
	pub fn pending_for_doc(&self, doc_id: DocId, kind: WorkKind) -> usize {
		self.pending_by_doc.get(&(doc_id, kind)).map(|c| c.load(Ordering::Relaxed)).unwrap_or(0)
	}

	/// Returns true if there is pending work.
	pub fn has_pending(&self) -> bool {
		!self.interactive.is_empty() || !self.background.is_empty()
	}

	/// Returns total pending work count.
	pub fn pending_count(&self) -> usize {
		self.interactive.len() + self.background.len()
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
	#[cfg(test)]
	pub fn dropped_total(&self) -> u64 {
		self.dropped_total
	}

	/// Drains completions within a time and count budget.
	pub async fn drain_budget(&mut self, budget: impl Into<DrainBudget>) -> DrainStats {
		if !self.has_pending() {
			return DrainStats::default();
		}

		let budget = budget.into();
		let start = Instant::now();
		let deadline = tokio::time::Instant::now() + budget.duration;
		let completed_before = self.completed_total;
		let mut remaining_completions = budget.max_completions;
		let mut panicked: u64 = 0;
		let mut cancelled: u64 = 0;
		let mut panic_sample: Option<String> = None;

		if remaining_completions == 0 {
			return DrainStats {
				completed: 0,
				pending: self.pending_count(),
				..DrainStats::default()
			};
		}

		// Helper closure to classify a JoinError.
		let classify = |e: tokio::task::JoinError, label: &str, panicked: &mut u64, cancelled: &mut u64, panic_sample: &mut Option<String>| {
			if e.is_panic() {
				*panicked += 1;
				let msg = xeno_worker::join_error_panic_message(e).unwrap_or_else(|| "<unknown panic>".to_string());
				if panic_sample.is_none() {
					*panic_sample = Some(format_panic_sample(&msg, 120));
				}
				tracing::error!(panic = %msg, "{label} work task panicked");
			} else {
				*cancelled += 1;
				tracing::warn!(?e, "{label} work task cancelled");
			}
		};

		// Phase 1: drain interactive queue.
		loop {
			// Fast-path: drain all ready completions.
			while remaining_completions > 0 {
				match self.interactive.try_join_next() {
					Some(Ok(())) => {
						self.completed_total += 1;
						remaining_completions -= 1;
					}
					Some(Err(e)) => {
						self.completed_total += 1;
						remaining_completions -= 1;
						classify(e, "interactive", &mut panicked, &mut cancelled, &mut panic_sample);
					}
					None => break,
				}
			}
			if remaining_completions == 0 || self.interactive.is_empty() {
				break;
			}
			// Wait for next completion or deadline.
			let notified = self.interactive_notify.notified();
			tokio::select! {
				biased;
				_ = notified => {}
				_ = tokio::time::sleep_until(deadline) => break,
			}
		}

		// Phase 2: drain background queue (only if interactive is empty).
		if self.interactive.is_empty() && remaining_completions > 0 {
			let _scope = self.gate.open_background_scope();

			loop {
				while remaining_completions > 0 {
					match self.background.try_join_next() {
						Some(Ok(())) => {
							self.completed_total += 1;
							remaining_completions -= 1;
						}
						Some(Err(e)) => {
							self.completed_total += 1;
							remaining_completions -= 1;
							classify(e, "background", &mut panicked, &mut cancelled, &mut panic_sample);
						}
						None => break,
					}
				}
				if remaining_completions == 0 || self.background.is_empty() {
					break;
				}
				let notified = self.background_notify.notified();
				tokio::select! {
					biased;
					_ = notified => {}
					_ = tokio::time::sleep_until(deadline) => break,
				}
			}
		}

		let completed_this_drain = self.completed_total - completed_before;
		let pending_after = self.pending_count();
		if completed_this_drain > 0 || pending_after > 0 {
			tracing::debug!(
				budget_ms = budget.duration.as_millis() as u64,
				elapsed_ms = start.elapsed().as_millis() as u64,
				completed = completed_this_drain,
				panicked,
				cancelled,
				budget_max = budget.max_completions,
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

		DrainStats {
			completed: completed_this_drain,
			panicked,
			cancelled,
			panic_sample,
			pending: pending_after,
		}
	}

	/// Drains all pending work.
	#[cfg(test)]
	pub async fn drain_all(&mut self) {
		while let Some(res) = self.interactive.join_next().await {
			if let Err(e) = res {
				tracing::error!(?e, "interactive work task failed during drain_all");
			}
			self.completed_total += 1;
		}
		{
			let _scope = self.gate.open_background_scope();
			while let Some(res) = self.background.join_next().await {
				if let Err(e) = res {
					tracing::error!(?e, "background work task failed during drain_all");
				}
				self.completed_total += 1;
			}
		}
	}
}

impl HookScheduler for WorkScheduler {
	fn schedule(&mut self, fut: HookBoxFuture, priority: HookPriority) {
		self.schedule(WorkItem {
			future: Box::pin(async move {
				let _ = fut.await;
			}),
			kind: WorkKind::Hook,
			priority,
			doc_id: None,
		});
	}
}

/// Budget for draining scheduled work completions.
#[derive(Debug, Clone, Copy)]
pub struct DrainBudget {
	pub duration: Duration,
	pub max_completions: usize,
}

impl DrainBudget {
	#[cfg(test)]
	pub fn new(duration: Duration, max_completions: usize) -> Self {
		Self { duration, max_completions }
	}
}

impl From<Duration> for DrainBudget {
	fn from(duration: Duration) -> Self {
		Self {
			duration,
			max_completions: usize::MAX,
		}
	}
}

/// Statistics from a drain cycle.
#[derive(Debug, Default, Clone)]
pub struct DrainStats {
	pub completed: u64,
	pub panicked: u64,
	pub cancelled: u64,
	pub panic_sample: Option<String>,
	pub pending: usize,
}

/// Truncates a string to at most `max_bytes` bytes on a char boundary.
fn truncate_utf8(s: &str, max_bytes: usize) -> String {
	if s.len() <= max_bytes {
		return s.to_string();
	}
	let mut idx = max_bytes;
	while idx > 0 && !s.is_char_boundary(idx) {
		idx -= 1;
	}
	format!("{}…", &s[..idx])
}

/// Extracts a single-line, truncated sample from a panic message.
fn format_panic_sample(msg: &str, max_bytes: usize) -> String {
	let first_line = msg.lines().next().unwrap_or("");
	truncate_utf8(first_line, max_bytes)
}
