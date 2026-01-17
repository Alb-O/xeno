//! Runtime metrics for observability.
//!
//! This module provides [`EditorMetrics`] for tracking hook and LSP statistics.
//! Metrics are emitted periodically via tracing and can be queried for debug displays.

use std::sync::atomic::{AtomicU64, Ordering};

/// Runtime metrics for hooks and LSP sync.
///
/// All counters use relaxed ordering for performance - exact counts aren't
/// critical, but trends should be visible.
#[derive(Debug, Default)]
pub struct EditorMetrics {
	/// Total LSP full sync sends.
	pub lsp_full_sync: AtomicU64,
	/// Total LSP incremental sync sends.
	pub lsp_incremental_sync: AtomicU64,
	/// Total LSP send errors.
	pub lsp_send_errors: AtomicU64,
	/// Total changes coalesced (removed by merging).
	pub lsp_coalesced: AtomicU64,
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
}

impl StatsSnapshot {
	/// Emits the stats as a tracing event.
	pub fn emit(&self) {
		tracing::info!(
			hooks_pending = self.hooks_pending,
			hooks_scheduled = self.hooks_scheduled,
			hooks_completed = self.hooks_completed,
			lsp_pending_docs = self.lsp_pending_docs,
			lsp_in_flight = self.lsp_in_flight,
			lsp_full_sync = self.lsp_full_sync,
			lsp_incremental_sync = self.lsp_incremental_sync,
			lsp_send_errors = self.lsp_send_errors,
			lsp_coalesced = self.lsp_coalesced,
			"editor.stats"
		);
	}
}
