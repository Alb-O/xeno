//! Unified LSP sync manager with owned pending state.
//!
//! [`LspSyncManager`] owns all pending changes per document and tracks:
//! * Pending incremental change batches
//! * Full-sync escalation and initial open state
//! * Debounce scheduling and retry timing
//! * Single in-flight sends with write timeout
//!
//! # Error Handling
//!
//! * Queue full / server not ready: retryable, payload retained
//! * Other errors: escalate to full sync, set retry delay

mod sync_state;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use ropey::Rope;
pub use sync_state::*;
use tokio::sync::mpsc;
use tracing::{debug, warn};
use xeno_lsp::DocumentSync;
use xeno_primitives::LspDocumentChange;

use super::coalesce::coalesce_changes;
use crate::core::document::DocumentId;
use crate::metrics::EditorMetrics;

/// Unified LSP sync manager owning all pending changes per document.
pub struct LspSyncManager {
	docs: HashMap<DocumentId, DocSyncState>,
	completion_rx: mpsc::UnboundedReceiver<FlushComplete>,
	completion_tx: mpsc::UnboundedSender<FlushComplete>,
}

impl Default for LspSyncManager {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for LspSyncManager {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LspSyncManager").field("docs", &self.docs.len()).finish()
	}
}

impl LspSyncManager {
	pub fn new() -> Self {
		let (completion_tx, completion_rx) = mpsc::unbounded_channel();
		Self {
			docs: HashMap::new(),
			completion_rx,
			completion_tx,
		}
	}

	pub fn ensure_tracked(&mut self, doc_id: DocumentId, config: LspDocumentConfig, version: u64) {
		if let Some(state) = self.docs.get_mut(&doc_id) {
			state.config = config;
			state.editor_version = state.editor_version.max(version);
			return;
		}

		debug!(
			doc_id = doc_id.0,
			path = ?config.path,
			version,
			"lsp.sync_manager.doc_track"
		);
		self.docs.insert(doc_id, DocSyncState::new(config, version));
	}

	pub fn reset_tracked(&mut self, doc_id: DocumentId, config: LspDocumentConfig, version: u64) {
		let generation = self.docs.get(&doc_id).map(|state| state.generation.wrapping_add(1)).unwrap_or(0);
		debug!(
			doc_id = doc_id.0,
			path = ?config.path,
			version,
			"lsp.sync_manager.doc_reset"
		);
		let mut state = DocSyncState::new(config, version);
		state.generation = generation;
		self.docs.insert(doc_id, state);
	}

	pub fn on_doc_close(&mut self, doc_id: DocumentId) {
		debug!(doc_id = doc_id.0, "lsp.sync_manager.doc_close");
		self.docs.remove(&doc_id);
	}

	/// Records edits for later sync. Untracked documents are silently ignored.
	pub fn on_doc_edit(&mut self, doc_id: DocumentId, prev_version: u64, new_version: u64, changes: Vec<LspDocumentChange>, bytes: usize) {
		if let Some(state) = self.docs.get_mut(&doc_id) {
			tracing::trace!(
				doc_id = doc_id.0,
				prev_version,
				new_version,
				change_count = changes.len(),
				bytes,
				"lsp.sync_manager.doc_edit"
			);
			state.record_changes(prev_version, new_version, changes, bytes);
		}
	}

	pub fn escalate_full(&mut self, doc_id: DocumentId) {
		if let Some(state) = self.docs.get_mut(&doc_id) {
			debug!(doc_id = doc_id.0, "lsp.sync_manager.escalate_full");
			state.escalate_full();
		}
	}

	/// Flushes one tracked document immediately, bypassing debounce.
	///
	/// Returns a receiver that resolves when the write barrier (if any) completes.
	pub fn flush_now(
		&mut self,
		now: Instant,
		doc_id: DocumentId,
		sync: &DocumentSync,
		metrics: &Arc<EditorMetrics>,
		snapshot: Option<(Rope, u64)>,
	) -> Option<tokio::sync::oneshot::Receiver<()>> {
		self.poll_completions();
		for state in self.docs.values_mut() {
			state.check_write_timeout(now, LSP_WRITE_TIMEOUT);
		}

		let state = self.docs.get_mut(&doc_id)?;
		if state.phase == SyncPhase::InFlight || state.retry_after.is_some_and(|t| now < t) {
			return None;
		}
		if state.pending_changes.is_empty() && !state.needs_full {
			return None;
		}

		let path = state.config.path.clone();
		let language = state.config.language.clone();
		let generation = state.generation;
		let use_full = state.needs_full || !state.config.supports_incremental;
		let (done_tx, done_rx) = tokio::sync::oneshot::channel();

		if use_full {
			let (content, _snapshot_version) = snapshot?;
			let snapshot_bytes = content.len_bytes() as u64;
			let _ = state.take_for_send(true);
			let sync = sync.clone();
			let tx = self.completion_tx.clone();
			let metrics = metrics.clone();

			tokio::spawn(async move {
				let snapshot = match tokio::task::spawn_blocking(move || content.to_string()).await {
					Ok(snapshot) => snapshot,
					Err(e) => {
						metrics.inc_send_error();
						warn!(
							doc_id = doc_id.0,
							path = ?path,
							error = %e,
							"lsp.sync_manager.snapshot_join_failed"
						);
						let _ = tx.send(FlushComplete {
							doc_id,
							generation,
							result: FlushResult::Failed,
							was_full: true,
						});
						let _ = done_tx.send(());
						return;
					}
				};
				metrics.add_snapshot_bytes(snapshot_bytes);

				let result = sync.notify_change_full_with_barrier_text(&path, &language, snapshot).await;
				let flush_result = match result {
					Ok(Some(barrier)) => match tokio::time::timeout(LSP_WRITE_TIMEOUT, barrier).await {
						Ok(Ok(())) => {
							metrics.inc_full_sync();
							FlushResult::Success
						}
						Ok(Err(_)) => {
							metrics.inc_send_error();
							FlushResult::Failed
						}
						Err(_) => {
							metrics.inc_send_error();
							FlushResult::Failed
						}
					},
					Ok(None) => {
						metrics.inc_full_sync();
						FlushResult::Success
					}
					Err(err) => {
						metrics.inc_send_error();
						FlushResult::from_error(&err)
					}
				};

				let _ = tx.send(FlushComplete {
					doc_id,
					generation,
					result: flush_result,
					was_full: true,
				});
				let _ = done_tx.send(());
			});
		} else {
			let (raw_changes, _) = state.take_for_send(false);
			let raw_count = raw_changes.len();
			let changes = coalesce_changes(raw_changes);
			let coalesced = raw_count.saturating_sub(changes.len());
			if coalesced > 0 {
				metrics.add_coalesced(coalesced as u64);
			}

			let sync = sync.clone();
			let tx = self.completion_tx.clone();
			let metrics = metrics.clone();

			tokio::spawn(async move {
				let result = sync.notify_change_incremental_no_content_with_barrier(&path, &language, changes).await;
				let flush_result = match result {
					Ok(Some(barrier)) => match tokio::time::timeout(LSP_WRITE_TIMEOUT, barrier).await {
						Ok(Ok(())) => {
							metrics.inc_incremental_sync();
							FlushResult::Success
						}
						Ok(Err(_)) => {
							metrics.inc_send_error();
							FlushResult::Failed
						}
						Err(_) => {
							metrics.inc_send_error();
							FlushResult::Failed
						}
					},
					Ok(None) => {
						metrics.inc_incremental_sync();
						FlushResult::Success
					}
					Err(err) => {
						metrics.inc_send_error();
						FlushResult::from_error(&err)
					}
				};

				let _ = tx.send(FlushComplete {
					doc_id,
					generation,
					result: flush_result,
					was_full: false,
				});
				let _ = done_tx.send(());
			});
		}

		if let Some(state) = self.docs.get_mut(&doc_id) {
			state.retry_after = None;
		}

		Some(done_rx)
	}

	#[cfg(test)]
	fn is_tracked(&self, doc_id: &DocumentId) -> bool {
		self.docs.contains_key(doc_id)
	}

	#[cfg(test)]
	fn doc_count(&self) -> usize {
		self.docs.len()
	}

	pub fn pending_count(&self) -> usize {
		self.docs
			.values()
			.filter(|s| s.phase == SyncPhase::Debouncing || !s.pending_changes.is_empty())
			.count()
	}

	pub fn in_flight_count(&self) -> usize {
		self.docs.values().filter(|s| s.phase == SyncPhase::InFlight).count()
	}

	fn poll_completions(&mut self) {
		while let Ok(complete) = self.completion_rx.try_recv() {
			if let Some(state) = self.docs.get_mut(&complete.doc_id) {
				if state.generation != complete.generation {
					tracing::debug!(
						doc_id = complete.doc_id.0,
						complete_generation = complete.generation,
						current_generation = state.generation,
						"lsp.sync_manager.drop_stale_completion"
					);
					continue;
				}
				state.mark_complete(complete.result, complete.was_full);
			}
		}
	}

	/// Flushes documents that are due for sync.
	pub fn tick<F>(&mut self, now: Instant, sync: &DocumentSync, metrics: &Arc<EditorMetrics>, snapshot_provider: F) -> FlushStats
	where
		F: Fn(DocumentId) -> Option<(Rope, u64)>,
	{
		self.poll_completions();

		for state in self.docs.values_mut() {
			state.check_write_timeout(now, LSP_WRITE_TIMEOUT);
		}

		let mut stats = FlushStats::default();

		let due_docs: Vec<_> = self
			.docs
			.iter()
			.filter(|(_, state)| state.is_due(now, LSP_DEBOUNCE))
			.map(|(&doc_id, _)| doc_id)
			.take(LSP_MAX_DOCS_PER_TICK)
			.collect();

		for doc_id in due_docs {
			if stats.flushed_docs >= LSP_MAX_DOCS_PER_TICK {
				break;
			}

			let Some(state) = self.docs.get_mut(&doc_id) else {
				continue;
			};

			let path = state.config.path.clone();
			let language = state.config.language.clone();
			let generation = state.generation;
			let use_full = state.needs_full || !state.config.supports_incremental;
			let editor_version = state.editor_version;

			if use_full {
				let Some((content, snapshot_version)) = snapshot_provider(doc_id) else {
					warn!(doc_id = doc_id.0, "lsp.sync_manager.no_snapshot");
					continue;
				};

				let _ = state.take_for_send(true);
				let snapshot_bytes = content.len_bytes() as u64;
				stats.full_syncs += 1;
				stats.snapshot_bytes += snapshot_bytes;

				debug!(
					doc_id = doc_id.0,
					path = ?path,
					mode = "full",
					snapshot_version,
					editor_version,
					"lsp.sync_manager.flush_start"
				);

				let sync = sync.clone();
				let tx = self.completion_tx.clone();
				let metrics = metrics.clone();

				tokio::spawn(async move {
					let start = Instant::now();
					let snapshot = match tokio::task::spawn_blocking(move || content.to_string()).await {
						Ok(snapshot) => snapshot,
						Err(e) => {
							metrics.inc_send_error();
							warn!(
								doc_id = doc_id.0,
								path = ?path,
								error = %e,
								"lsp.sync_manager.snapshot_join_failed"
							);
							let _ = tx.send(FlushComplete {
								doc_id,
								generation,
								result: FlushResult::Failed,
								was_full: true,
							});
							return;
						}
					};
					metrics.add_snapshot_bytes(snapshot_bytes);

					let result = sync.notify_change_full_with_barrier_text(&path, &language, snapshot).await;
					let latency_ms = start.elapsed().as_millis() as u64;

					let flush_result = match result {
						Ok(Some(barrier)) => match tokio::time::timeout(LSP_WRITE_TIMEOUT, barrier).await {
							Ok(Ok(())) => {
								metrics.inc_full_sync();
								debug!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, "lsp.sync_manager.flush_done");
								FlushResult::Success
							}
							Ok(Err(_)) => {
								metrics.inc_send_error();
								warn!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, "lsp.sync_manager.barrier_dropped");
								FlushResult::Failed
							}
							Err(_) => {
								metrics.inc_send_error();
								warn!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, "lsp.sync_manager.barrier_timeout");
								FlushResult::Failed
							}
						},
						Ok(None) => {
							metrics.inc_full_sync();
							debug!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, "lsp.sync_manager.flush_done");
							FlushResult::Success
						}
						Err(err) => {
							metrics.inc_send_error();
							let classified = FlushResult::from_error(&err);
							if classified == FlushResult::Retryable {
								debug!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, error = ?err, "lsp.sync_manager.flush_retryable");
							} else {
								warn!(doc_id = doc_id.0, path = ?path, mode = "full", latency_ms, error = ?err, "lsp.sync_manager.flush_failed");
							}
							classified
						}
					};

					let _ = tx.send(FlushComplete {
						doc_id,
						generation,
						result: flush_result,
						was_full: true,
					});
				});
			} else {
				let (raw_changes, _) = state.take_for_send(false);
				let raw_count = raw_changes.len();
				let changes = coalesce_changes(raw_changes);
				let coalesced = raw_count.saturating_sub(changes.len());

				if coalesced > 0 {
					metrics.add_coalesced(coalesced as u64);
				}

				stats.incremental_syncs += 1;

				debug!(
					doc_id = doc_id.0,
					path = ?path,
					mode = "incremental",
					raw_count,
					change_count = changes.len(),
					coalesced,
					editor_version,
					"lsp.sync_manager.flush_start"
				);

				let sync = sync.clone();
				let tx = self.completion_tx.clone();
				let metrics = metrics.clone();

				tokio::spawn(async move {
					let start = Instant::now();
					let result = sync.notify_change_incremental_no_content_with_barrier(&path, &language, changes).await;
					let latency_ms = start.elapsed().as_millis() as u64;

					let flush_result = match result {
						Ok(Some(barrier)) => match tokio::time::timeout(LSP_WRITE_TIMEOUT, barrier).await {
							Ok(Ok(())) => {
								metrics.inc_incremental_sync();
								debug!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, "lsp.sync_manager.flush_done");
								FlushResult::Success
							}
							Ok(Err(_)) => {
								metrics.inc_send_error();
								warn!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, "lsp.sync_manager.barrier_dropped");
								FlushResult::Failed
							}
							Err(_) => {
								metrics.inc_send_error();
								warn!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, "lsp.sync_manager.barrier_timeout");
								FlushResult::Failed
							}
						},
						Ok(None) => {
							metrics.inc_incremental_sync();
							debug!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, "lsp.sync_manager.flush_done");
							FlushResult::Success
						}
						Err(err) => {
							metrics.inc_send_error();
							let classified = FlushResult::from_error(&err);
							if classified == FlushResult::Retryable {
								debug!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, error = ?err, "lsp.sync_manager.flush_retryable");
							} else {
								warn!(doc_id = doc_id.0, path = ?path, mode = "incremental", latency_ms, error = ?err, "lsp.sync_manager.flush_failed");
							}
							classified
						}
					};

					let _ = tx.send(FlushComplete {
						doc_id,
						generation,
						result: flush_result,
						was_full: false,
					});
				});
			}

			if let Some(state) = self.docs.get_mut(&doc_id) {
				state.retry_after = None;
			}

			stats.flushed_docs += 1;
		}

		stats
	}
}

#[cfg(test)]
mod tests;
