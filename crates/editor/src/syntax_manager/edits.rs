use super::*;

impl SyntaxManager {
	/// Resets the syntax state for a document, clearing the current tree and history.
	pub fn reset_syntax(&mut self, doc_id: DocumentId) {
		let entry = self.entry_mut(doc_id);
		if entry.slot.current.is_some() {
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

	/// Records an edit and attempts an immediate incremental update.
	///
	/// This is the primary path for interactive typing. It attempts to update
	/// the resident syntax tree synchronously (with a 10ms timeout). If the
	/// update fails or is debounced, it accumulates the changes for a
	/// background parse.
	///
	/// # Invariants
	///
	/// - Sync incremental updates are ONLY allowed if the resident tree's version
	///   matches the version immediately preceding this edit.
	/// - If alignment is lost, we fallback to a full reparse in the background.
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

		if let Some(current) = &entry.slot.current
			&& current.is_partial()
		{
			// Never attempt incremental updates on a partial tree. Keep the
			// partial tree installed for continuity and force immediate catch-up.
			entry.slot.pending_incremental = None;
			entry.sched.force_no_debounce = true;
			return;
		}

		let Some(syntax) = entry.slot.current.as_mut() else {
			entry.slot.pending_incremental = None;
			return;
		};

		if doc_version == 0 {
			entry.slot.pending_incremental = None;
			return;
		}
		let version_before = doc_version - 1;

		// Manage pending incremental window
		match entry.slot.pending_incremental.take() {
			Some(mut pending) => {
				if entry.slot.tree_doc_version != Some(pending.base_tree_doc_version) {
					// Tree has diverged from pending base; invalid window
					entry.slot.pending_incremental = None;
				} else {
					pending.composed = pending.composed.compose(changeset.clone());
					entry.slot.pending_incremental = Some(pending);
				}
			}
			None => {
				// Only start a pending window if the tree matches the version before this edit.
				if let Some(tree_v) = entry.slot.tree_doc_version
					&& tree_v == version_before
				{
					entry.slot.pending_incremental = Some(PendingIncrementalEdits {
						base_tree_doc_version: tree_v,
						old_rope: old_rope.clone(),
						composed: changeset.clone(),
					});
				}
			}
		}

		let Some(pending) = entry.slot.pending_incremental.as_ref() else {
			return;
		};

		// Attempt sync catch-up from pending base to latest rope
		let opts = SyntaxOptions {
			parse_timeout: SYNC_TIMEOUT,
			..syntax.opts()
		};

		if syntax
			.update_from_changeset(
				pending.old_rope.slice(..),
				new_rope.slice(..),
				&pending.composed,
				loader,
				opts,
			)
			.is_ok()
		{
			entry.slot.pending_incremental = None;
			entry.slot.dirty = false;
			entry.slot.tree_doc_version = Some(doc_version);
			Self::mark_updated(&mut entry.slot);
		} else {
			tracing::debug!(
				?doc_id,
				"Sync incremental update failed; keeping pending for catch-up"
			);
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
