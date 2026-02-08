use std::time::{Duration, Instant};

use tokio::task::JoinSet;
use xeno_registry::hooks::HookPriority;

use super::state::{BACKGROUND_DROP_THRESHOLD, BACKLOG_HIGH_WATER, WorkScheduler};
use super::types::{DocId, WorkItem, WorkKind};

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
				let guard = self.gate.enter_interactive();
				self.interactive.spawn(async move {
					let _guard = guard;
					item.future.await;
				});
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
				let gate = self.gate.clone();
				self.background.spawn(async move {
					gate.wait_for_background().await;
					item.future.await;
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

	/// Cancels pending work for a specific document and kind.
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
	pub async fn drain_budget(&mut self, budget: Duration) {
		if !self.has_pending() {
			return;
		}

		let start = Instant::now();
		let deadline = start + budget;
		let completed_before = self.completed_total;

		while Instant::now() < deadline && !self.interactive.is_empty() {
			let remaining = deadline.saturating_duration_since(Instant::now());
			match tokio::time::timeout(remaining, self.interactive.join_next()).await {
				Ok(Some(Ok(()))) => {
					self.completed_total += 1;
				}
				Ok(Some(Err(e))) => {
					self.completed_total += 1;
					tracing::error!(?e, "interactive work task failed");
				}
				_ => break,
			}
		}

		if self.interactive.is_empty() {
			let _scope = self.gate.open_background_scope();

			while Instant::now() < deadline && !self.background.is_empty() {
				let remaining = deadline.saturating_duration_since(Instant::now());
				match tokio::time::timeout(remaining, self.background.join_next()).await {
					Ok(Some(Ok(()))) => {
						self.completed_total += 1;
					}
					Ok(Some(Err(e))) => {
						self.completed_total += 1;
						tracing::error!(?e, "background work task failed");
					}
					_ => break,
				}
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

	/// Drops all pending background work.
	pub fn drop_background(&mut self) {
		let count = self.background.len();
		if count > 0 {
			self.background.abort_all();
			self.background = JoinSet::new();
			self.dropped_total += count as u64;
			tracing::info!(dropped = count, "dropped all background work");
		}
	}
}
