//! Runtime metrics for observability.
//!
//! This module provides [`crate::metrics::EditorMetrics`] for tracking hook and LSP statistics.
//! Metrics are emitted periodically via tracing and can be queried for debug displays.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::runtime::RuntimeDrainExitReason;
use crate::runtime::work_queue::{RuntimeWorkKindOldestAgesMs, RuntimeWorkKindTag, RuntimeWorkSource};

/// Runtime metrics for hooks and LSP sync.
///
/// All counters use relaxed ordering for performance - exact counts aren't
/// critical, but trends should be visible.
#[derive(Debug, Default)]
pub struct EditorMetrics {
	/// Hooks completed in the last tick.
	pub hooks_completed_tick: AtomicU64,
	/// Hooks pending after the last tick.
	pub hooks_pending_tick: AtomicU64,
	/// Total LSP full sync sends.
	pub lsp_full_sync: AtomicU64,
	/// Total LSP incremental sync sends.
	pub lsp_incremental_sync: AtomicU64,
	/// Total LSP send errors.
	pub lsp_send_errors: AtomicU64,
	/// Total changes coalesced (removed by merging).
	pub lsp_coalesced: AtomicU64,
	/// Total bytes snapshotted for full document syncs.
	pub lsp_snapshot_bytes: AtomicU64,
	/// Total scheduler tasks completed.
	pub worker_drained_completed: AtomicU64,
	/// Total scheduler tasks that panicked.
	pub worker_drained_panicked: AtomicU64,
	/// Total scheduler tasks that were cancelled.
	pub worker_drained_cancelled: AtomicU64,
	/// Full syncs scheduled in the last tick.
	pub lsp_full_sync_tick: AtomicU64,
	/// Incremental syncs scheduled in the last tick.
	pub lsp_incremental_sync_tick: AtomicU64,
	/// Snapshot bytes scheduled in the last tick.
	pub lsp_snapshot_bytes_tick: AtomicU64,
	/// Latest runtime event queue depth.
	pub runtime_event_queue_depth: AtomicU64,
	/// Latest runtime work queue depth.
	pub runtime_work_queue_depth: AtomicU64,
	/// Latest observed oldest runtime work age.
	pub runtime_work_oldest_age_ms: AtomicU64,
	/// Total runtime drain rounds executed.
	pub runtime_drain_rounds_executed: AtomicU64,
	/// Total runtime drain exits due to idle.
	pub runtime_drain_exit_idle_total: AtomicU64,
	/// Total runtime drain exits due to round cap.
	pub runtime_drain_exit_round_cap_total: AtomicU64,
	/// Total runtime drain exits due to quit.
	pub runtime_drain_exit_quit_total: AtomicU64,
	/// Total runtime drain exits due to budget cap.
	pub runtime_drain_exit_budget_cap_total: AtomicU64,
	/// Total runtime work items drained.
	pub runtime_work_drained_total: AtomicU64,
	/// Latest observed event-to-directive latency.
	pub runtime_event_to_directive_latency_ms: AtomicU64,
}

impl EditorMetrics {
	/// Creates a new metrics instance.
	pub fn new() -> Self {
		Self::default()
	}

	/// Increments the full sync counter.
	pub fn inc_full_sync(&self) {
		self.lsp_full_sync.fetch_add(1, Ordering::Relaxed);
	}

	/// Increments the incremental sync counter.
	pub fn inc_incremental_sync(&self) {
		self.lsp_incremental_sync.fetch_add(1, Ordering::Relaxed);
	}

	/// Increments the send error counter.
	pub fn inc_send_error(&self) {
		self.lsp_send_errors.fetch_add(1, Ordering::Relaxed);
	}

	/// Adds to the coalesced counter.
	pub fn add_coalesced(&self, count: u64) {
		self.lsp_coalesced.fetch_add(count, Ordering::Relaxed);
	}

	/// Adds to the snapshot bytes counter.
	pub fn add_snapshot_bytes(&self, bytes: u64) {
		self.lsp_snapshot_bytes.fetch_add(bytes, Ordering::Relaxed);
	}

	/// Records worker drain stats from a scheduler drain cycle.
	pub fn record_worker_drain(&self, completed: u64, panicked: u64, cancelled: u64) {
		self.worker_drained_completed.fetch_add(completed, Ordering::Relaxed);
		self.worker_drained_panicked.fetch_add(panicked, Ordering::Relaxed);
		self.worker_drained_cancelled.fetch_add(cancelled, Ordering::Relaxed);
	}

	/// Returns total panicked worker tasks.
	pub fn worker_panicked_total(&self) -> u64 {
		self.worker_drained_panicked.load(Ordering::Relaxed)
	}

	/// Records per-tick hook drain stats.
	pub fn record_hook_tick(&self, completed: u64, pending: usize) {
		self.hooks_completed_tick.store(completed, Ordering::Relaxed);
		self.hooks_pending_tick.store(pending as u64, Ordering::Relaxed);
	}

	/// Records per-tick LSP sync counts.
	pub fn record_lsp_tick(&self, full_syncs: u64, incremental_syncs: u64, snapshot_bytes: u64) {
		self.lsp_full_sync_tick.store(full_syncs, Ordering::Relaxed);
		self.lsp_incremental_sync_tick.store(incremental_syncs, Ordering::Relaxed);
		self.lsp_snapshot_bytes_tick.store(snapshot_bytes, Ordering::Relaxed);
	}

	/// Returns the current full sync count.
	pub fn full_sync_count(&self) -> u64 {
		self.lsp_full_sync.load(Ordering::Relaxed)
	}

	/// Returns the current incremental sync count.
	pub fn incremental_sync_count(&self) -> u64 {
		self.lsp_incremental_sync.load(Ordering::Relaxed)
	}

	/// Returns the current send error count.
	pub fn send_error_count(&self) -> u64 {
		self.lsp_send_errors.load(Ordering::Relaxed)
	}

	/// Returns the current coalesced count.
	pub fn coalesced_count(&self) -> u64 {
		self.lsp_coalesced.load(Ordering::Relaxed)
	}

	/// Returns the current snapshot byte count.
	pub fn snapshot_bytes_count(&self) -> u64 {
		self.lsp_snapshot_bytes.load(Ordering::Relaxed)
	}

	/// Returns hooks completed in the last tick.
	pub fn hooks_completed_tick_count(&self) -> u64 {
		self.hooks_completed_tick.load(Ordering::Relaxed)
	}

	/// Returns hooks pending after the last tick.
	pub fn hooks_pending_tick_count(&self) -> u64 {
		self.hooks_pending_tick.load(Ordering::Relaxed)
	}

	/// Returns the last tick full sync count.
	pub fn full_sync_tick_count(&self) -> u64 {
		self.lsp_full_sync_tick.load(Ordering::Relaxed)
	}

	/// Returns the last tick incremental sync count.
	pub fn incremental_sync_tick_count(&self) -> u64 {
		self.lsp_incremental_sync_tick.load(Ordering::Relaxed)
	}

	/// Returns the last tick snapshot byte count.
	pub fn snapshot_bytes_tick_count(&self) -> u64 {
		self.lsp_snapshot_bytes_tick.load(Ordering::Relaxed)
	}

	/// Records `runtime_event_queue_depth`.
	pub fn record_runtime_event_queue_depth(&self, depth: u64) {
		self.runtime_event_queue_depth.store(depth, Ordering::Relaxed);
		tracing::trace!(metric = "runtime_event_queue_depth", value = depth, "metric.runtime");
	}

	/// Records `runtime_work_queue_depth`.
	pub fn record_runtime_work_queue_depth(&self, depth: u64) {
		self.runtime_work_queue_depth.store(depth, Ordering::Relaxed);
		tracing::trace!(metric = "runtime_work_queue_depth", value = depth, "metric.runtime");
	}

	/// Records `runtime_work_oldest_age_ms` per kind and as a top-level gauge.
	pub fn record_runtime_work_oldest_age_ms_by_kind(&self, ages: RuntimeWorkKindOldestAgesMs) {
		let mut max_age = 0u64;

		if let Some(age_ms) = ages.invocation_ms {
			max_age = max_age.max(age_ms);
			tracing::trace!(metric = "runtime_work_oldest_age_ms", kind = "invocation", value = age_ms, "metric.runtime");
		}
		if let Some(age_ms) = ages.overlay_commit_ms {
			max_age = max_age.max(age_ms);
			tracing::trace!(metric = "runtime_work_oldest_age_ms", kind = "overlay_commit", value = age_ms, "metric.runtime");
		}
		#[cfg(feature = "lsp")]
		if let Some(age_ms) = ages.workspace_edit_ms {
			max_age = max_age.max(age_ms);
			tracing::trace!(metric = "runtime_work_oldest_age_ms", kind = "workspace_edit", value = age_ms, "metric.runtime");
		}

		self.runtime_work_oldest_age_ms.store(max_age, Ordering::Relaxed);
	}

	/// Records `runtime_drain_rounds_executed`.
	pub fn record_runtime_drain_rounds_executed(&self, rounds: u64) {
		self.runtime_drain_rounds_executed.fetch_add(rounds, Ordering::Relaxed);
		tracing::trace!(metric = "runtime_drain_rounds_executed", value = rounds, "metric.runtime");
	}

	/// Records `runtime_drain_exit_reason_total`.
	pub fn record_runtime_drain_exit_reason(&self, reason: RuntimeDrainExitReason) {
		match reason {
			RuntimeDrainExitReason::Idle => {
				self.runtime_drain_exit_idle_total.fetch_add(1, Ordering::Relaxed);
			}
			RuntimeDrainExitReason::RoundCap => {
				self.runtime_drain_exit_round_cap_total.fetch_add(1, Ordering::Relaxed);
			}
			RuntimeDrainExitReason::Quit => {
				self.runtime_drain_exit_quit_total.fetch_add(1, Ordering::Relaxed);
			}
			RuntimeDrainExitReason::BudgetCap => {
				self.runtime_drain_exit_budget_cap_total.fetch_add(1, Ordering::Relaxed);
			}
		}
		tracing::trace!(metric = "runtime_drain_exit_reason_total", ?reason, value = 1u64, "metric.runtime");
	}

	/// Records `runtime_work_drained_total{kind,source}`.
	pub fn record_runtime_work_drained_total(&self, kind: RuntimeWorkKindTag, source: Option<RuntimeWorkSource>) {
		self.runtime_work_drained_total.fetch_add(1, Ordering::Relaxed);

		let source_label = source.map_or("internal", |src| match src {
			RuntimeWorkSource::ActionEffect => "action_effect",
			RuntimeWorkSource::Overlay => "overlay",
			RuntimeWorkSource::CommandOps => "command_ops",
			RuntimeWorkSource::NuHookDispatch => "nu_hook_dispatch",
			RuntimeWorkSource::NuScheduledMacro => "nu_scheduled_macro",
		});
		let kind_label = match kind {
			RuntimeWorkKindTag::Invocation => "invocation",
			RuntimeWorkKindTag::OverlayCommit => "overlay_commit",
			#[cfg(feature = "lsp")]
			RuntimeWorkKindTag::WorkspaceEdit => "workspace_edit",
		};

		tracing::trace!(
			metric = "runtime_work_drained_total",
			kind = kind_label,
			source = source_label,
			value = 1u64,
			"metric.runtime"
		);
	}

	/// Records `runtime_event_to_directive_latency_ms`.
	pub fn record_runtime_event_to_directive_latency_ms(&self, latency_ms: u64) {
		self.runtime_event_to_directive_latency_ms.store(latency_ms, Ordering::Relaxed);
		tracing::trace!(metric = "runtime_event_to_directive_latency_ms", value = latency_ms, "metric.runtime");
	}
}

/// Nu runtime and hook pipeline health snapshot.
#[derive(Debug, Clone, Default)]
pub struct NuStats {
	pub runtime_loaded: bool,
	pub script_path: Option<String>,
	pub executor_alive: bool,
	pub hook_phase: &'static str,
	pub hook_queue_len: usize,
	pub hook_in_flight: Option<(u64, u64, String)>,
	pub runtime_work_queue_len: usize,
	pub hook_dropped_total: u64,
	pub runtime_epoch: u64,
	pub hook_eval_seq_next: u64,
}

/// Snapshot of current editor statistics for display.
#[derive(Debug, Clone, Default)]
pub struct StatsSnapshot {
	/// Number of pending hooks.
	pub hooks_pending: usize,
	/// Total hooks scheduled.
	pub hooks_scheduled: u64,
	/// Total hooks completed.
	pub hooks_completed: u64,
	/// Hooks completed in the last tick.
	pub hooks_completed_tick: u64,
	/// Hooks pending after the last tick.
	pub hooks_pending_tick: u64,
	/// Documents with pending LSP changes.
	pub lsp_pending_docs: usize,
	/// Documents with in-flight LSP sends.
	pub lsp_in_flight: usize,
	/// Total full syncs sent.
	pub lsp_full_sync: u64,
	/// Total incremental syncs sent.
	pub lsp_incremental_sync: u64,
	/// Total send errors.
	pub lsp_send_errors: u64,
	/// Total changes coalesced.
	pub lsp_coalesced: u64,
	/// Total bytes snapshotted for full syncs.
	pub lsp_snapshot_bytes: u64,
	/// Full syncs scheduled in the last tick.
	pub lsp_full_sync_tick: u64,
	/// Incremental syncs scheduled in the last tick.
	pub lsp_incremental_sync_tick: u64,
	/// Snapshot bytes scheduled in the last tick.
	pub lsp_snapshot_bytes_tick: u64,
	/// Nu runtime and hook pipeline health.
	pub nu: NuStats,
}

impl StatsSnapshot {
	/// Emits the stats as a tracing event.
	pub fn emit(&self) {
		tracing::info!(
			hooks_pending = self.hooks_pending,
			hooks_scheduled = self.hooks_scheduled,
			hooks_completed = self.hooks_completed,
			hooks_completed_tick = self.hooks_completed_tick,
			hooks_pending_tick = self.hooks_pending_tick,
			lsp_pending_docs = self.lsp_pending_docs,
			lsp_in_flight = self.lsp_in_flight,
			lsp_full_sync = self.lsp_full_sync,
			lsp_incremental_sync = self.lsp_incremental_sync,
			lsp_send_errors = self.lsp_send_errors,
			lsp_coalesced = self.lsp_coalesced,
			lsp_snapshot_bytes = self.lsp_snapshot_bytes,
			lsp_full_sync_tick = self.lsp_full_sync_tick,
			lsp_incremental_sync_tick = self.lsp_incremental_sync_tick,
			lsp_snapshot_bytes_tick = self.lsp_snapshot_bytes_tick,
			nu_runtime = self.nu.runtime_loaded,
			nu_executor = self.nu.executor_alive,
			nu_hook_phase = self.nu.hook_phase,
			nu_hook_queue = self.nu.hook_queue_len,
			nu_runtime_work_queue = self.nu.runtime_work_queue_len,
			nu_hook_dropped = self.nu.hook_dropped_total,
			nu_epoch = self.nu.runtime_epoch,
			nu_eval_seq_next = self.nu.hook_eval_seq_next,
			"editor.stats"
		);
	}
}
