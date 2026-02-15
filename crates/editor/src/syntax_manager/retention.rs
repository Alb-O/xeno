use super::*;

impl SyntaxManager {
	/// Invariant enforcement: Checks if a completed background parse should be installed into a slot.
	pub(crate) fn should_install_completed_parse(
		done_version: u64,
		current_tree_version: Option<u64>,
		requested_version: u64,
		target_version: u64,
		slot_dirty: bool,
	) -> bool {
		if let Some(v) = current_tree_version
			&& done_version < v
		{
			return false;
		}

		// Never install results older than the last request to avoid "flicker"
		// where an old slow parse finishes after a newer faster parse was already requested.
		if done_version < requested_version {
			return false;
		}

		let version_match = done_version == target_version;
		if version_match {
			return true;
		}

		if current_tree_version.is_none() {
			return true;
		}

		// Continuity installs while dirty should only apply if they advance the
		// resident version. Reinstalling the same stale version causes extra
		// repaint churn without improving highlight fidelity. Callers may apply
		// additional continuity guards (for example, projection alignment checks).
		slot_dirty && current_tree_version.is_some_and(|v| done_version > v)
	}

	/// Evaluates if the retention policy allows installing a new syntax tree.
	pub(super) fn retention_allows_install(now: Instant, st: &DocSched, policy: RetentionPolicy, hotness: SyntaxHotness) -> bool {
		if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			return true;
		}
		match policy {
			RetentionPolicy::Keep => true,
			RetentionPolicy::DropWhenHidden => false,
			RetentionPolicy::DropAfter(ttl) => now.duration_since(st.last_visible_at) <= ttl,
		}
	}

	/// Invariant enforcement: Bumps the change counter after a state change.
	pub(crate) fn mark_updated(state: &mut SyntaxSlot) {
		state.updated = true;
		state.change_id = state.change_id.wrapping_add(1);
	}

	/// Sweeps all document entries for retention, dropping trees for cold docs that exceed TTL.
	///
	/// Runs independently of `ensure_syntax` so that cold/clean docs that are never
	/// polled still have their trees evicted. Also invalidates inflight tasks and flushes
	/// completed queues for cold hidden-parse-disabled docs, preventing unbounded memory
	/// accumulation from completed `Syntax` trees that would never be installed.
	/// Returns true if any artifact was dropped.
	pub fn sweep_retention(&mut self, now: Instant, hotness: impl Fn(DocumentId) -> SyntaxHotness) -> bool {
		let mut any_dropped = false;
		let doc_ids: Vec<_> = self.entries.keys().copied().collect();
		for doc_id in doc_ids {
			let entry = self.entries.get_mut(&doc_id).unwrap();
			let Some(tier) = entry.last_tier else { continue };
			let cfg = self.policy.cfg(tier);
			let h = hotness(doc_id);

			// For cold docs that shouldn't parse when hidden, invalidate any pending
			// work and flush the completed queue to prevent memory accumulation from
			// completed syntax trees that would never be installed.
			if h == SyntaxHotness::Cold && !cfg.parse_when_hidden && (entry.sched.any_active() || !entry.sched.completed.is_empty()) {
				entry.sched.invalidate();
			}

			if Self::apply_retention(
				now,
				&entry.sched,
				cfg.retention_hidden_full,
				cfg.retention_hidden_viewport,
				h,
				&mut entry.slot,
				doc_id,
			) {
				if entry.sched.any_active() {
					entry.sched.invalidate();
				}
				any_dropped = true;
			}
		}
		any_dropped
	}

	/// Applies memory retention rules separately for full tree and viewport cache.
	///
	/// Returns true if any artifact was dropped.
	pub(crate) fn apply_retention(
		now: Instant,
		st: &DocSched,
		full_policy: RetentionPolicy,
		viewport_policy: RetentionPolicy,
		hotness: SyntaxHotness,
		state: &mut SyntaxSlot,
		_doc_id: DocumentId,
	) -> bool {
		if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			return false;
		}

		let mut dropped = false;

		// Full tree retention
		let drop_full = match full_policy {
			RetentionPolicy::Keep => false,
			RetentionPolicy::DropWhenHidden => state.full.is_some() || state.dirty,
			RetentionPolicy::DropAfter(ttl) => (state.full.is_some() || state.dirty) && now.duration_since(st.last_visible_at) > ttl,
		};
		if drop_full {
			state.drop_full();
			state.dirty = false;
			state.pending_incremental = None;
			Self::mark_updated(state);
			dropped = true;
		}

		// Viewport cache retention
		let drop_viewport = match viewport_policy {
			RetentionPolicy::Keep => false,
			RetentionPolicy::DropWhenHidden => state.viewport_cache.has_any(),
			RetentionPolicy::DropAfter(ttl) => state.viewport_cache.has_any() && now.duration_since(st.last_visible_at) > ttl,
		};
		if drop_viewport {
			state.drop_viewport();
			Self::mark_updated(state);
			dropped = true;
		}

		dropped
	}
}
