//! Actor-owned LSP sync manager.
//!
//! [`LspSyncManager`] is a non-blocking command handle over a supervised
//! `lsp.sync` actor. Actor state is authoritative; the handle exposes
//! eventually-consistent counters and tick deltas for editor telemetry.
//! Use [`LspSyncManager::shutdown`] for explicit bounded teardown.

mod sync_state;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use ropey::Rope;
pub use sync_state::*;
use tokio::sync::oneshot;
use tracing::{debug, warn};
use xeno_lsp::{BarrierMode, ChangePayload, ChangeRequest, DocumentSync};
use xeno_primitives::{DocumentId, LspDocumentChange};

use super::coalesce::coalesce_changes;
use crate::metrics::EditorMetrics;

#[derive(Debug)]
enum SendWork {
	Full { content: Rope, snapshot_bytes: u64 },
	Incremental { changes: Vec<LspDocumentChange> },
}

impl SendWork {
	fn was_full(&self) -> bool {
		matches!(self, Self::Full { .. })
	}

	fn mode(&self) -> &'static str {
		if self.was_full() { "full" } else { "incremental" }
	}
}

struct SendTaskInput {
	command_port: xeno_worker::ActorCommandPort<LspSyncCmd>,
	sync: DocumentSync,
	metrics: Arc<EditorMetrics>,
	doc_id: DocumentId,
	generation: u64,
	path: PathBuf,
	language: String,
	work: SendWork,
	done_tx: Option<oneshot::Sender<()>>,
}

struct SendDispatch {
	generation: u64,
	path: PathBuf,
	language: String,
	work: SendWork,
}

enum LspSyncCmd {
	EnsureTracked {
		doc_id: DocumentId,
		config: LspDocumentConfig,
		version: u64,
	},
	ResetTracked {
		doc_id: DocumentId,
		config: LspDocumentConfig,
		version: u64,
	},
	Close {
		doc_id: DocumentId,
	},
	Edit {
		doc_id: DocumentId,
		prev_version: u64,
		new_version: u64,
		changes: Vec<LspDocumentChange>,
		bytes: usize,
	},
	EscalateFull {
		doc_id: DocumentId,
	},
	Tick {
		now: Instant,
		sync: DocumentSync,
		metrics: Arc<EditorMetrics>,
		full_snapshots: HashMap<DocumentId, (Rope, u64)>,
	},
	FlushNow {
		now: Instant,
		doc_id: DocumentId,
		sync: DocumentSync,
		metrics: Arc<EditorMetrics>,
		snapshot: Option<(Rope, u64)>,
		done_tx: Option<oneshot::Sender<()>>,
	},
	SendComplete(FlushComplete),
	#[cfg(test)]
	CrashForTest,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum LspSyncEvent {
	FlushCompleted { doc_id: DocumentId, result: FlushResult, was_full: bool },
	RetryScheduled { doc_id: DocumentId },
	EscalatedFull { doc_id: DocumentId },
	WriteTimeout { doc_id: DocumentId },
}

#[derive(Debug, Clone)]
pub struct LspSyncShutdownReport {
	pub actor: xeno_worker::ActorShutdownReport,
}

#[derive(Clone, Default)]
struct LspSyncShared {
	tracked_docs: HashSet<DocumentId>,
	due_full_docs: HashSet<DocumentId>,
	pending_count: usize,
	in_flight_count: usize,
	pending_tick_stats: FlushStats,
}

struct LspSyncActor {
	docs: HashMap<DocumentId, DocSyncState>,
	command_port: Arc<std::sync::OnceLock<xeno_worker::ActorCommandPort<LspSyncCmd>>>,
	shared: Arc<RwLock<LspSyncShared>>,
}

impl LspSyncActor {
	fn ensure_tracked(&mut self, doc_id: DocumentId, config: LspDocumentConfig, version: u64) {
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

	fn reset_tracked(&mut self, doc_id: DocumentId, config: LspDocumentConfig, version: u64) {
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

	fn on_doc_close(&mut self, doc_id: DocumentId) {
		debug!(doc_id = doc_id.0, "lsp.sync_manager.doc_close");
		self.docs.remove(&doc_id);
	}

	fn on_doc_edit(&mut self, doc_id: DocumentId, prev_version: u64, new_version: u64, changes: Vec<LspDocumentChange>, bytes: usize) {
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

	fn escalate_full(&mut self, doc_id: DocumentId, ctx: &mut xeno_worker::ActorContext<LspSyncEvent>) {
		if let Some(state) = self.docs.get_mut(&doc_id) {
			debug!(doc_id = doc_id.0, "lsp.sync_manager.escalate_full");
			state.escalate_full();
			ctx.emit(LspSyncEvent::EscalatedFull { doc_id });
		}
	}

	fn check_write_timeouts(&mut self, now: Instant, ctx: &mut xeno_worker::ActorContext<LspSyncEvent>) {
		for (&doc_id, state) in &mut self.docs {
			if state.check_write_timeout(now, LSP_WRITE_TIMEOUT) {
				ctx.emit(LspSyncEvent::WriteTimeout { doc_id });
			}
		}
	}

	fn check_write_timeouts_inner(&mut self, now: Instant) {
		for state in self.docs.values_mut() {
			state.check_write_timeout(now, LSP_WRITE_TIMEOUT);
		}
	}

	fn sync_shared_counts(&self, now: Instant) {
		let tracked_docs: HashSet<DocumentId> = self.docs.keys().copied().collect();
		let due_full_docs: HashSet<DocumentId> = self
			.docs
			.iter()
			.filter(|(_, state)| state.is_due(now, LSP_DEBOUNCE) && (state.needs_full || !state.config.supports_incremental))
			.map(|(&doc_id, _)| doc_id)
			.take(LSP_MAX_DOCS_PER_TICK)
			.collect();
		let pending_count = self
			.docs
			.values()
			.filter(|s| s.phase == SyncPhase::Debouncing || !s.pending_changes.is_empty())
			.count();
		let in_flight_count = self.docs.values().filter(|s| s.phase == SyncPhase::InFlight).count();
		let mut shared = self.shared.write();
		shared.tracked_docs = tracked_docs;
		shared.due_full_docs = due_full_docs;
		shared.pending_count = pending_count;
		shared.in_flight_count = in_flight_count;
	}

	fn add_tick_stats(&self, stats: FlushStats) {
		let mut shared = self.shared.write();
		shared.pending_tick_stats.flushed_docs = shared.pending_tick_stats.flushed_docs.saturating_add(stats.flushed_docs);
		shared.pending_tick_stats.full_syncs = shared.pending_tick_stats.full_syncs.saturating_add(stats.full_syncs);
		shared.pending_tick_stats.incremental_syncs = shared.pending_tick_stats.incremental_syncs.saturating_add(stats.incremental_syncs);
		shared.pending_tick_stats.snapshot_bytes = shared.pending_tick_stats.snapshot_bytes.saturating_add(stats.snapshot_bytes);
	}

	fn command_port(&self) -> Option<xeno_worker::ActorCommandPort<LspSyncCmd>> {
		self.command_port.get().cloned()
	}

	fn spawn_send_task(input: SendTaskInput) {
		let SendTaskInput {
			command_port,
			sync,
			metrics,
			doc_id,
			generation,
			path,
			language,
			work,
			done_tx,
		} = input;

		xeno_worker::spawn(xeno_worker::TaskClass::Background, async move {
			let mode = work.mode();
			let was_full = work.was_full();
			let start = Instant::now();

			let send_result = match work {
				SendWork::Full { content, snapshot_bytes } => {
					let snapshot = match xeno_worker::spawn_blocking(xeno_worker::TaskClass::CpuBlocking, move || content.to_string())
						.await
					{
						Ok(snapshot) => snapshot,
						Err(e) => {
							metrics.inc_send_error();
							warn!(
								doc_id = doc_id.0,
								path = ?path,
								error = %e,
								"lsp.sync_manager.snapshot_join_failed"
							);
							let _ = command_port.send(LspSyncCmd::SendComplete(FlushComplete {
								doc_id,
								generation,
								result: FlushResult::Failed,
								was_full,
							}));
							if let Some(done_tx) = done_tx {
								let _ = done_tx.send(());
							}
							return;
						}
					};
					metrics.add_snapshot_bytes(snapshot_bytes);
					sync.send_change(
						ChangeRequest::full_text(&path, &language, snapshot)
							.with_barrier(BarrierMode::Tracked)
							.with_open_if_needed(true),
					)
					.await
				}
				SendWork::Incremental { changes } => {
					sync.send_change(ChangeRequest {
						path: &path,
						language: &language,
						payload: ChangePayload::Incremental(changes),
						barrier: BarrierMode::Tracked,
						open_if_needed: false,
					})
					.await
				}
			};

			let latency_ms = start.elapsed().as_millis() as u64;
			let flush_result = match send_result {
				Ok(dispatch) => match dispatch.barrier {
					Some(barrier) => match tokio::time::timeout(LSP_WRITE_TIMEOUT, barrier).await {
						Ok(Ok(())) => {
							if was_full {
								metrics.inc_full_sync();
							} else {
								metrics.inc_incremental_sync();
							}
							debug!(doc_id = doc_id.0, path = ?path, mode, latency_ms, "lsp.sync_manager.flush_done");
							FlushResult::Success
						}
						Ok(Err(_)) => {
							metrics.inc_send_error();
							warn!(doc_id = doc_id.0, path = ?path, mode, latency_ms, "lsp.sync_manager.barrier_dropped");
							FlushResult::Failed
						}
						Err(_) => {
							metrics.inc_send_error();
							warn!(doc_id = doc_id.0, path = ?path, mode, latency_ms, "lsp.sync_manager.barrier_timeout");
							FlushResult::Failed
						}
					},
					None => {
						if was_full {
							metrics.inc_full_sync();
						} else {
							metrics.inc_incremental_sync();
						}
						debug!(doc_id = doc_id.0, path = ?path, mode, latency_ms, "lsp.sync_manager.flush_done");
						FlushResult::Success
					}
				},
				Err(err) => {
					metrics.inc_send_error();
					let classified = FlushResult::from_error(&err);
					if classified == FlushResult::Retryable {
						debug!(doc_id = doc_id.0, path = ?path, mode, latency_ms, error = ?err, "lsp.sync_manager.flush_retryable");
					} else {
						warn!(doc_id = doc_id.0, path = ?path, mode, latency_ms, error = ?err, "lsp.sync_manager.flush_failed");
					}
					classified
				}
			};

			let _ = command_port.send(LspSyncCmd::SendComplete(FlushComplete {
				doc_id,
				generation,
				result: flush_result,
				was_full,
			}));
			if let Some(done_tx) = done_tx {
				let _ = done_tx.send(());
			}
		});
	}

	fn take_send_dispatch(
		state: &mut DocSyncState,
		doc_id: DocumentId,
		snapshot: Option<(Rope, u64)>,
		metrics: &Arc<EditorMetrics>,
		stats: Option<&mut FlushStats>,
	) -> Option<SendDispatch> {
		let path = state.config.path.clone();
		let language = state.config.language.clone();
		let generation = state.generation;
		let use_full = state.needs_full || !state.config.supports_incremental;
		let editor_version = state.editor_version;

		let work = if use_full {
			let Some((content, snapshot_version)) = snapshot else {
				warn!(doc_id = doc_id.0, "lsp.sync_manager.no_snapshot");
				return None;
			};
			let _ = state.take_for_send(true);
			let snapshot_bytes = content.len_bytes() as u64;
			if let Some(stats) = stats {
				stats.full_syncs = stats.full_syncs.saturating_add(1);
				stats.snapshot_bytes = stats.snapshot_bytes.saturating_add(snapshot_bytes);
			}

			debug!(
				doc_id = doc_id.0,
				path = ?path,
				mode = "full",
				snapshot_version,
				editor_version,
				"lsp.sync_manager.flush_start"
			);

			SendWork::Full { content, snapshot_bytes }
		} else {
			let (raw_changes, _) = state.take_for_send(false);
			let raw_count = raw_changes.len();
			let changes = coalesce_changes(raw_changes);
			let coalesced = raw_count.saturating_sub(changes.len());
			if coalesced > 0 {
				metrics.add_coalesced(coalesced as u64);
			}
			if let Some(stats) = stats {
				stats.incremental_syncs = stats.incremental_syncs.saturating_add(1);
			}

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

			SendWork::Incremental { changes }
		};

		Some(SendDispatch {
			generation,
			path,
			language,
			work,
		})
	}

	fn flush_now(
		&mut self,
		now: Instant,
		doc_id: DocumentId,
		sync: &DocumentSync,
		metrics: &Arc<EditorMetrics>,
		snapshot: Option<(Rope, u64)>,
		done_tx: Option<oneshot::Sender<()>>,
	) {
		self.check_write_timeouts_inner(now);
		let Some(command_port) = self.command_port() else {
			if let Some(done_tx) = done_tx {
				let _ = done_tx.send(());
			}
			return;
		};
		let Some(state) = self.docs.get_mut(&doc_id) else {
			if let Some(done_tx) = done_tx {
				let _ = done_tx.send(());
			}
			return;
		};
		if state.phase == SyncPhase::InFlight || state.retry_after.is_some_and(|t| now < t) {
			if let Some(done_tx) = done_tx {
				let _ = done_tx.send(());
			}
			return;
		}
		if state.pending_changes.is_empty() && !state.needs_full {
			if let Some(done_tx) = done_tx {
				let _ = done_tx.send(());
			}
			return;
		}
		let Some(dispatch) = Self::take_send_dispatch(state, doc_id, snapshot, metrics, None) else {
			if let Some(done_tx) = done_tx {
				let _ = done_tx.send(());
			}
			return;
		};

		Self::spawn_send_task(SendTaskInput {
			command_port,
			sync: sync.clone(),
			metrics: metrics.clone(),
			doc_id,
			generation: dispatch.generation,
			path: dispatch.path,
			language: dispatch.language,
			work: dispatch.work,
			done_tx,
		});

		if let Some(state) = self.docs.get_mut(&doc_id) {
			state.retry_after = None;
		}
	}

	fn tick(&mut self, now: Instant, sync: &DocumentSync, metrics: &Arc<EditorMetrics>, full_snapshots: HashMap<DocumentId, (Rope, u64)>) -> FlushStats {
		self.check_write_timeouts_inner(now);
		let mut stats = FlushStats::default();
		let Some(command_port) = self.command_port() else {
			return stats;
		};

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

			let snapshot = full_snapshots.get(&doc_id).cloned();
			let Some(dispatch) = Self::take_send_dispatch(state, doc_id, snapshot, metrics, Some(&mut stats)) else {
				continue;
			};

			Self::spawn_send_task(SendTaskInput {
				command_port: command_port.clone(),
				sync: sync.clone(),
				metrics: metrics.clone(),
				doc_id,
				generation: dispatch.generation,
				path: dispatch.path,
				language: dispatch.language,
				work: dispatch.work,
				done_tx: None,
			});

			if let Some(state) = self.docs.get_mut(&doc_id) {
				state.retry_after = None;
			}

			stats.flushed_docs += 1;
		}

		stats
	}
}

#[async_trait::async_trait]
impl xeno_worker::Actor for LspSyncActor {
	type Cmd = LspSyncCmd;
	type Evt = LspSyncEvent;

	async fn handle(&mut self, cmd: Self::Cmd, ctx: &mut xeno_worker::ActorContext<Self::Evt>) -> Result<xeno_worker::ActorFlow, String> {
		let mut snapshot_now = Instant::now();
		match cmd {
			LspSyncCmd::EnsureTracked { doc_id, config, version } => self.ensure_tracked(doc_id, config, version),
			LspSyncCmd::ResetTracked { doc_id, config, version } => self.reset_tracked(doc_id, config, version),
			LspSyncCmd::Close { doc_id } => self.on_doc_close(doc_id),
			LspSyncCmd::Edit {
				doc_id,
				prev_version,
				new_version,
				changes,
				bytes,
			} => self.on_doc_edit(doc_id, prev_version, new_version, changes, bytes),
			LspSyncCmd::EscalateFull { doc_id } => self.escalate_full(doc_id, ctx),
			LspSyncCmd::Tick {
				now,
				sync,
				metrics,
				full_snapshots,
			} => {
				snapshot_now = now;
				self.check_write_timeouts(now, ctx);
				let stats = self.tick(now, &sync, &metrics, full_snapshots);
				self.add_tick_stats(stats);
			}
			LspSyncCmd::FlushNow {
				now,
				doc_id,
				sync,
				metrics,
				snapshot,
				done_tx,
			} => {
				snapshot_now = now;
				self.check_write_timeouts(now, ctx);
				self.flush_now(now, doc_id, &sync, &metrics, snapshot, done_tx);
			}
			LspSyncCmd::SendComplete(complete) => {
				if let Some(state) = self.docs.get_mut(&complete.doc_id) {
					if state.generation != complete.generation {
						tracing::debug!(
							doc_id = complete.doc_id.0,
							complete_generation = complete.generation,
							current_generation = state.generation,
							"lsp.sync_manager.drop_stale_completion"
						);
					} else {
						let was_failed = complete.result == FlushResult::Failed;
						let was_retry = complete.result == FlushResult::Retryable;
						state.mark_complete(complete.result, complete.was_full);
						if was_failed {
							ctx.emit(LspSyncEvent::EscalatedFull { doc_id: complete.doc_id });
						}
						if was_retry {
							ctx.emit(LspSyncEvent::RetryScheduled { doc_id: complete.doc_id });
						}
						ctx.emit(LspSyncEvent::FlushCompleted {
							doc_id: complete.doc_id,
							result: complete.result,
							was_full: complete.was_full,
						});
					}
				}
			}
			#[cfg(test)]
			LspSyncCmd::CrashForTest => return Err("lsp.sync crash test hook".to_string()),
		}

		self.sync_shared_counts(snapshot_now);
		Ok(xeno_worker::ActorFlow::Continue)
	}
}

/// Actor-backed LSP sync manager command handle.
pub struct LspSyncManager {
	shared: Arc<RwLock<LspSyncShared>>,
	ingress: xeno_worker::ActorCommandIngress<LspSyncCmd, LspSyncEvent>,
}

impl Default for LspSyncManager {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for LspSyncManager {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LspSyncManager")
			.field("tracked_docs", &self.shared.read().tracked_docs.len())
			.finish()
	}
}

impl LspSyncManager {
	pub fn new() -> Self {
		let shared = Arc::new(RwLock::new(LspSyncShared::default()));
		let command_port = Arc::new(std::sync::OnceLock::<xeno_worker::ActorCommandPort<LspSyncCmd>>::new());
		let actor = Arc::new(
			xeno_worker::ActorRuntime::spawn(
				xeno_worker::ActorSpec::new("lsp.sync", xeno_worker::TaskClass::Background, {
					let command_port = Arc::clone(&command_port);
					let shared = Arc::clone(&shared);
					move || LspSyncActor {
						docs: HashMap::new(),
						command_port: Arc::clone(&command_port),
						shared: Arc::clone(&shared),
					}
				})
				.supervisor(xeno_worker::ActorSupervisorSpec::default()
					.restart(xeno_worker::ActorRestartPolicy::OnFailure {
						max_restarts: 3,
						backoff: Duration::from_millis(50),
					})
					.event_buffer(64)),
			),
		);
		let ingress = xeno_worker::ActorCommandIngress::with_capacity(xeno_worker::TaskClass::Background, Arc::clone(&actor), 4096);
		let _ = command_port.set(ingress.port());

		Self { shared, ingress }
	}

	fn send(&self, cmd: LspSyncCmd) {
		let _ = self.ingress.send(cmd);
	}

	pub fn ensure_tracked(&mut self, doc_id: DocumentId, config: LspDocumentConfig, version: u64) {
		self.send(LspSyncCmd::EnsureTracked { doc_id, config, version });
	}

	pub fn reset_tracked(&mut self, doc_id: DocumentId, config: LspDocumentConfig, version: u64) {
		self.send(LspSyncCmd::ResetTracked { doc_id, config, version });
	}

	pub fn on_doc_close(&mut self, doc_id: DocumentId) {
		self.send(LspSyncCmd::Close { doc_id });
	}

	/// Records edits for later sync. Untracked documents are silently ignored.
	pub fn on_doc_edit(&mut self, doc_id: DocumentId, prev_version: u64, new_version: u64, changes: Vec<LspDocumentChange>, bytes: usize) {
		self.send(LspSyncCmd::Edit {
			doc_id,
			prev_version,
			new_version,
			changes,
			bytes,
		});
	}

	pub fn escalate_full(&mut self, doc_id: DocumentId) {
		self.send(LspSyncCmd::EscalateFull { doc_id });
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
		let (done_tx, done_rx) = tokio::sync::oneshot::channel();
		self.send(LspSyncCmd::FlushNow {
			now,
			doc_id,
			sync: sync.clone(),
			metrics: metrics.clone(),
			snapshot,
			done_tx: Some(done_tx),
		});
		Some(done_rx)
	}

	pub fn pending_count(&self) -> usize {
		self.shared.read().pending_count
	}

	pub fn in_flight_count(&self) -> usize {
		self.shared.read().in_flight_count
	}

	/// Flushes documents that are due for sync.
	///
	/// Returns stats accumulated from previously completed actor ticks.
	pub fn tick<F>(&mut self, now: Instant, sync: &DocumentSync, metrics: &Arc<EditorMetrics>, snapshot_provider: F) -> FlushStats
	where
		F: Fn(DocumentId) -> Option<(Rope, u64)>,
	{
		let stats = {
			let mut shared = self.shared.write();
			let stats = shared.pending_tick_stats;
			shared.pending_tick_stats = FlushStats::default();
			stats
		};

		let due_full_docs: Vec<DocumentId> = self.shared.read().due_full_docs.iter().copied().collect();
		let doc_count = due_full_docs.len().max(1);
		let mut full_snapshots = HashMap::with_capacity(doc_count);
		for doc_id in due_full_docs {
			if let Some(snapshot) = snapshot_provider(doc_id) {
				full_snapshots.insert(doc_id, snapshot);
			}
		}

		self.send(LspSyncCmd::Tick {
			now,
			sync: sync.clone(),
			metrics: metrics.clone(),
			full_snapshots,
		});

		stats
	}

	#[allow(dead_code)]
	fn has_tracked_doc(&self, doc_id: &DocumentId) -> bool {
		self.shared.read().tracked_docs.contains(doc_id)
	}

	#[cfg(test)]
	fn is_tracked(&self, doc_id: &DocumentId) -> bool {
		self.has_tracked_doc(doc_id)
	}

	#[cfg(test)]
	fn doc_count(&self) -> usize {
		self.shared.read().tracked_docs.len()
	}

	pub async fn shutdown(&self, mode: xeno_worker::ActorShutdownMode) -> LspSyncShutdownReport {
		let actor = self.ingress.shutdown(mode).await;
		LspSyncShutdownReport { actor }
	}

	#[cfg(test)]
	fn restart_count(&self) -> usize {
		self.ingress.actor().restart_count()
	}

	#[cfg(test)]
	fn crash_for_test(&self) {
		let _ = self.ingress.send(LspSyncCmd::CrashForTest);
	}
}

#[cfg(test)]
mod tests;
