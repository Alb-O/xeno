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
		// repaint churn without improving highlight fidelity.
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
