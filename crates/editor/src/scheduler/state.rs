use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use futures::stream::FuturesUnordered;

use super::types::{DocId, WorkKind};

/// Unified scheduler for async work.
///
/// Routes all async work (hooks, LSP, indexing, watchers) through a single
/// backpressure system with explicit budgets and priorities.
pub struct WorkScheduler {
	/// Interactive work (must complete).
	pub(super) interactive: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
	/// Background work (can be dropped).
	pub(super) background: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
	/// Pending work items by (doc_id, kind) for cancellation.
	pub(super) pending_by_doc: HashMap<(DocId, WorkKind), usize>,
	/// Total work scheduled.
	pub(super) scheduled_total: u64,
	/// Total work completed.
	pub(super) completed_total: u64,
	/// Background work dropped due to backlog.
	pub(super) dropped_total: u64,
}

/// High-water mark for total pending work before warning.
pub(super) const BACKLOG_HIGH_WATER: usize = 500;

/// Threshold at which background work is dropped.
pub(super) const BACKGROUND_DROP_THRESHOLD: usize = 200;

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
