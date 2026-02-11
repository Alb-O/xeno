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
		let has_current = current_tree_version.is_some();

		version_match || slot_dirty || !has_current
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

	/// Invariant enforcement: Bumps the syntax version after a state change.
	pub(crate) fn mark_updated(state: &mut SyntaxSlot) {
		state.updated = true;
		state.version = state.version.wrapping_add(1);
	}

	/// Invariant enforcement: Applies memory retention rules to a syntax slot.
	pub(crate) fn apply_retention(
		now: Instant,
		st: &DocSched,
		policy: RetentionPolicy,
		hotness: SyntaxHotness,
		state: &mut SyntaxSlot,
		_doc_id: DocumentId,
	) -> bool {
		if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			return false;
		}

		match policy {
			RetentionPolicy::Keep => false,
			RetentionPolicy::DropWhenHidden => {
				if state.current.is_some() || state.dirty {
					state.drop_tree();
					state.dirty = false;
					state.pending_incremental = None;
					Self::mark_updated(state);
					true
				} else {
					false
				}
			}
			RetentionPolicy::DropAfter(ttl) => {
				if (state.current.is_some() || state.dirty) && now.duration_since(st.last_visible_at) > ttl {
					state.drop_tree();
					state.dirty = false;
					Self::mark_updated(state);
					true
				} else {
					false
				}
			}
		}
	}
}
