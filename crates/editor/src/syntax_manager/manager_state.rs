use super::*;

pub(super) struct DocEntry {
	pub(super) sched: DocSched,
	pub(super) slot: SyntaxSlot,
	/// Last known tier for retention sweep (updated on each ensure_syntax call).
	pub(super) last_tier: Option<policy::SyntaxTier>,
}

impl DocEntry {
	pub(super) fn new(now: Instant) -> Self {
		Self {
			sched: DocSched::new(now),
			slot: SyntaxSlot::default(),
			last_tier: None,
		}
	}
}

/// Top-level scheduler for background syntax parsing and results storage.
///
/// The [`SyntaxManager`] enforces global concurrency limits via a semaphore and
/// manages per-document state, including incremental updates and tiered policies.
/// It integrates with the editor tick and render loops to ensure monotonic tree
/// installation and prompt permit release.
pub struct SyntaxManager {
	/// Global configuration.
	pub(super) cfg: SyntaxManagerCfg,
	/// Tiered policy mapping file size to specific configurations.
	pub(super) policy: TieredSyntaxPolicy,
	/// Runtime metrics for adaptive scheduling.
	pub(super) metrics: SyntaxMetrics,
	/// Global semaphore limiting concurrent background parse tasks.
	pub(super) permits: Arc<Semaphore>,
	/// Per-document scheduling and syntax state.
	pub(super) entries: HashMap<DocumentId, DocEntry>,
	/// Pluggable parsing engine (abstracted for tests).
	pub(super) engine: Arc<dyn SyntaxEngine>,
	/// Collector for background tasks.
	pub(super) collector: TaskCollector,
}
