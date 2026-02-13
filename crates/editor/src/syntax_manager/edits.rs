use super::*;

impl SyntaxManager {
	/// Resets the syntax state for a document, clearing all trees and history.
	pub fn reset_syntax(&mut self, doc_id: DocumentId) {
		let entry = self.entry_mut(doc_id);
		if entry.slot.has_any_tree() {
			entry.slot.drop_tree();
			Self::mark_updated(&mut entry.slot);
		}
		entry.slot.dirty = true;
		entry.slot.pending_incremental = None;
		entry.sched.invalidate();
	}

	/// Marks a document as dirty, triggering a reparse on the next poll.
	pub fn mark_dirty(&mut self, doc_id: DocumentId) {
		self.entry_mut(doc_id).slot.dirty = true;
	}

	/// Records an edit for debounce scheduling without changeset data.
	pub fn note_edit(&mut self, doc_id: DocumentId, source: EditSource) {
		let now = Instant::now();
		let entry = self.entry_mut(doc_id);
		entry.sched.last_edit_at = now;
		entry.slot.dirty = true;
		if source == EditSource::History {
			entry.sched.force_no_debounce = true;
		}
	}

	/// Records an edit and attempts an immediate incremental update on the full tree.
	///
	/// Viewport trees accumulate edits for highlight projection but never
	/// receive sync incremental updates (their partial source windows make
	/// tree-sitter incremental editing invalid).
	///
	/// # Invariants
	///
	/// - Sync incremental updates are ONLY allowed on full (non-partial) trees
	///   whose version matches the version immediately preceding this edit.
	/// - If alignment is lost, changes accumulate for a background parse.
	pub fn note_edit_incremental(
		&mut self,
		doc_id: DocumentId,
		doc_version: u64,
		old_rope: &Rope,
		new_rope: &Rope,
		changeset: &ChangeSet,
		loader: &LanguageLoader,
		source: EditSource,
	) {
		const SYNC_TIMEOUT: Duration = Duration::from_millis(10);

		let now = Instant::now();
		let entry = self.entry_mut(doc_id);
		entry.sched.last_edit_at = now;
		entry.slot.dirty = true;

		if source == EditSource::History {
			entry.sched.force_no_debounce = true;
		}

		// Determine which tree doc version to anchor the pending window to.
		// Prefer full tree; fall back to viewport tree for projection-only tracking.
		let anchor_version = entry.slot.full_doc_version.or(entry.slot.viewport_cache.best_doc_version());
		let has_full_tree = entry.slot.full.is_some();

		if anchor_version.is_none() {
			entry.slot.pending_incremental = None;
			return;
		}

		if doc_version == 0 {
			entry.slot.pending_incremental = None;
			return;
		}
		let version_before = doc_version - 1;

		// Manage pending incremental window.
		match entry.slot.pending_incremental.take() {
			Some(mut pending) => {
				if anchor_version != Some(pending.base_tree_doc_version) {
					entry.slot.pending_incremental = None;
				} else {
					pending.composed = pending.composed.compose(changeset.clone());
					entry.slot.pending_incremental = Some(pending);
				}
			}
			None => {
				if let Some(anchor_v) = anchor_version
					&& anchor_v == version_before
				{
					entry.slot.pending_incremental = Some(PendingIncrementalEdits {
						base_tree_doc_version: anchor_v,
						old_rope: old_rope.clone(),
						composed: changeset.clone(),
					});
				}
			}
		}

		// Only attempt sync incremental on full (non-partial) trees.
		if !has_full_tree {
			entry.sched.force_no_debounce = true;
			return;
		}

		let Some(pending) = entry.slot.pending_incremental.as_ref() else {
			return;
		};

		// Only sync if the pending window is anchored to the full tree's version.
		if entry.slot.full_doc_version != Some(pending.base_tree_doc_version) {
			return;
		}

		let syntax = entry.slot.full.as_mut().unwrap();
		let opts = SyntaxOptions {
			parse_timeout: SYNC_TIMEOUT,
			..syntax.opts()
		};

		if syntax
			.update_from_changeset(pending.old_rope.slice(..), new_rope.slice(..), &pending.composed, loader, opts)
			.is_ok()
		{
			entry.slot.pending_incremental = None;
			entry.slot.dirty = false;
			entry.slot.full_doc_version = Some(doc_version);
			Self::mark_updated(&mut entry.slot);
		} else {
			tracing::debug!(?doc_id, "Sync incremental update failed; keeping pending for catch-up");
		}
	}

	/// Cleans up tracking state for a closed document.
	pub fn on_document_close(&mut self, doc_id: DocumentId) {
		self.forget_doc(doc_id);
	}

	/// Removes all tracking state and pending tasks for a document.
	pub fn forget_doc(&mut self, doc_id: DocumentId) {
		if let Some(mut entry) = self.entries.remove(&doc_id) {
			entry.sched.invalidate();
		}
	}
}
