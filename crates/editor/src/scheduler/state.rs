use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use tokio::sync::Notify;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::super::execution_gate::ExecutionGate;
use super::types::{DocId, WorkKind};

/// Unified scheduler for async work.
///
/// Routes all async work (hooks, LSP, indexing, watchers) through a single
/// backpressure system with explicit budgets and priorities.
pub struct WorkScheduler {
	/// Interactive work (must complete).
	pub(super) interactive: JoinSet<()>,
	/// Background work (can be dropped).
	pub(super) background: JoinSet<()>,
	/// Wakeup notify for interactive completions (including panics/cancels).
	pub(super) interactive_notify: Arc<Notify>,
	/// Wakeup notify for background completions (including panics/cancels).
	pub(super) background_notify: Arc<Notify>,
	/// Gate for enforcing strict ordering.
	pub(super) gate: ExecutionGate,
	/// Per-document cancellation tokens for active (non-cancelled) documents.
	pub(super) doc_cancel: HashMap<DocId, CancellationToken>,
	/// Bounded LRU set of cancelled doc ids for sticky cancellation.
	pub(super) cancelled_docs: HashSet<DocId>,
	/// Insertion order for LRU eviction of `cancelled_docs`.
	pub(super) cancelled_docs_order: VecDeque<DocId>,
	/// Per-(doc, kind) cancellation tokens for cancel/coalesce semantics.
	pub(super) kind_cancel: HashMap<(DocId, WorkKind), CancellationToken>,
	/// Pending work counts by (doc_id, kind), decremented automatically via drop guard.
	pub(super) pending_by_doc: HashMap<(DocId, WorkKind), Arc<AtomicUsize>>,
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

/// Maximum number of cancelled doc ids to retain for sticky cancellation.
pub(super) const CANCELLED_DOC_LRU_CAP: usize = 4096;

impl Default for WorkScheduler {
	fn default() -> Self {
		Self {
			interactive: JoinSet::new(),
			background: JoinSet::new(),
			interactive_notify: Arc::new(Notify::new()),
			background_notify: Arc::new(Notify::new()),
			gate: ExecutionGate::new(),
			doc_cancel: HashMap::new(),
			cancelled_docs: HashSet::new(),
			cancelled_docs_order: VecDeque::new(),
			kind_cancel: HashMap::new(),
			pending_by_doc: HashMap::new(),
			scheduled_total: 0,
			completed_total: 0,
			dropped_total: 0,
		}
	}
}
