//! Runtime metrics for observability.
//!
//! This module provides [`crate::metrics::EditorMetrics`] for tracking hook and LSP statistics.
//! Metrics are emitted periodically via tracing and can be queried for debug displays.

use std::sync::atomic::{AtomicU64, Ordering};

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
	/// Full syncs scheduled in the last tick.
	pub lsp_full_sync_tick: AtomicU64,
	/// Incremental syncs scheduled in the last tick.
	pub lsp_incremental_sync_tick: AtomicU64,
	/// Snapshot bytes scheduled in the last tick.
	pub lsp_snapshot_bytes_tick: AtomicU64,
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
	pub hook_failed_total: u64,
	pub runtime_epoch: u64,
	pub hook_eval_seq_next: u64,
	pub macro_depth: u8,
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
			nu_hook_failed = self.nu.hook_failed_total,
			nu_epoch = self.nu.runtime_epoch,
			nu_eval_seq_next = self.nu.hook_eval_seq_next,
			nu_macro_depth = self.nu.macro_depth,
			"editor.stats"
		);
	}
}
