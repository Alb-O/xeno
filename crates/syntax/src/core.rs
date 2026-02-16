use super::*;

impl Default for SyntaxManager {
	/// Creates a new manager with default concurrency limits.
	fn default() -> Self {
		Self::new(SyntaxManagerCfg::default())
	}
}

impl SyntaxManager {
	pub fn new(cfg: SyntaxManagerCfg) -> Self {
		let max_concurrency = cfg.max_concurrency.max(1);
		let cfg = SyntaxManagerCfg {
			max_concurrency,
			viewport_reserve: cfg.viewport_reserve.min(max_concurrency.saturating_sub(1)),
		};
		Self {
			policy: TieredSyntaxPolicy::default(),
			metrics: SyntaxMetrics::new(),
			permits: Arc::new(Semaphore::new(max_concurrency)),
			entries: HashMap::new(),
			engine: Arc::new(RealSyntaxEngine),
			collector: TaskCollector::new(),
			warm_docs: RecentDocLru::default(),
			cfg,
		}
	}

	#[cfg(any(test, doc))]
	pub fn new_with_engine(cfg: SyntaxManagerCfg, engine: Arc<dyn SyntaxEngine>) -> Self {
		let max_concurrency = cfg.max_concurrency.max(1);
		let cfg = SyntaxManagerCfg {
			max_concurrency,
			viewport_reserve: cfg.viewport_reserve.min(max_concurrency.saturating_sub(1)),
		};
		Self {
			policy: TieredSyntaxPolicy::test_default(),
			metrics: SyntaxMetrics::new(),
			permits: Arc::new(Semaphore::new(max_concurrency)),
			entries: HashMap::new(),
			engine,
			collector: TaskCollector::new(),
			warm_docs: RecentDocLru::default(),
			cfg,
		}
	}

	/// Clears the dirty flag for a document without going through a parse cycle.
	///
	/// Test-only helper that enables sibling modules (e.g. `invariants`) to
	/// manipulate private [`SyntaxSlot`] state for edge-case coverage.
	#[cfg(test)]
	pub(crate) fn force_clean(&mut self, doc_id: DocumentId) {
		self.entry_mut(doc_id).slot.dirty = false;
	}

	pub fn set_policy(&mut self, policy: TieredSyntaxPolicy) {
		assert!(
			policy.s_max_bytes_inclusive <= policy.m_max_bytes_inclusive,
			"TieredSyntaxPolicy: s_max ({}) must be <= m_max ({})",
			policy.s_max_bytes_inclusive,
			policy.m_max_bytes_inclusive
		);
		self.policy = policy;
	}

	/// Returns true if the document's last-known tier disables parsing when hidden.
	///
	/// Used by call sites to skip polling cold docs that don't need background work.
	pub fn is_hidden_parse_disabled(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.is_some_and(|entry| entry.last_tier.is_some_and(|tier| !self.policy.cfg(tier).parse_when_hidden))
	}

	/// Marks a document as recently visible for warm retention behavior.
	pub fn note_visible_doc(&mut self, doc_id: DocumentId) {
		self.warm_docs.touch(doc_id);
	}

	/// Returns true when the document is in the warm visibility set.
	pub fn is_warm_doc(&self, doc_id: DocumentId) -> bool {
		self.warm_docs.contains(doc_id)
	}

	pub(super) fn entry_mut(&mut self, doc_id: DocumentId) -> &mut DocEntry {
		self.entries.entry(doc_id).or_insert_with(|| DocEntry::new(Instant::now()))
	}
}
