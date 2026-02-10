use super::*;

impl SyntaxManager {
	pub fn has_syntax(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.and_then(|e| e.slot.current.as_ref())
			.is_some()
	}

	pub fn is_dirty(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.map(|e| e.slot.dirty)
			.unwrap_or(false)
	}

	pub fn syntax_for_doc(&self, doc_id: DocumentId) -> Option<&Syntax> {
		self.entries
			.get(&doc_id)
			.and_then(|e| e.slot.current.as_ref())
	}

	pub fn syntax_version(&self, doc_id: DocumentId) -> u64 {
		self.entries
			.get(&doc_id)
			.map(|e| e.slot.version)
			.unwrap_or(0)
	}

	/// Returns the document version that the installed syntax tree corresponds to.
	#[cfg(test)]
	pub(crate) fn syntax_doc_version(&self, doc_id: DocumentId) -> Option<u64> {
		self.entries.get(&doc_id)?.slot.tree_doc_version
	}

	/// Returns projection context for mapping stale tree highlights onto current text.
	///
	/// Returns `None` when tree and target versions already match, or when no
	/// aligned pending window exists.
	pub(crate) fn highlight_projection_ctx(
		&self,
		doc_id: DocumentId,
		doc_version: u64,
	) -> Option<HighlightProjectionCtx<'_>> {
		let entry = self.entries.get(&doc_id)?;
		let tree_doc_version = entry.slot.tree_doc_version?;
		if tree_doc_version == doc_version {
			return None;
		}

		let pending = entry.slot.pending_incremental.as_ref()?;
		if pending.base_tree_doc_version != tree_doc_version {
			return None;
		}

		Some(HighlightProjectionCtx {
			tree_doc_version,
			target_doc_version: doc_version,
			base_rope: &pending.old_rope,
			composed_changes: &pending.composed,
		})
	}

	/// Returns the document-global byte coverage of the installed syntax tree.
	pub fn syntax_coverage(&self, doc_id: DocumentId) -> Option<std::ops::Range<u32>> {
		self.entries.get(&doc_id)?.slot.coverage.clone()
	}

	/// Returns true if a background task is currently active for a document (even if detached).
	#[cfg(test)]
	pub(crate) fn has_inflight_task(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.is_some_and(|e| e.sched.active_task.is_some())
	}

	pub fn has_pending(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.is_some_and(|d| d.sched.active_task.is_some() && !d.sched.active_task_detached)
	}

	pub fn pending_count(&self) -> usize {
		self.entries
			.values()
			.filter(|d| d.sched.active_task.is_some() && !d.sched.active_task_detached)
			.count()
	}

	pub fn pending_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries
			.iter()
			.filter(|(_, d)| d.sched.active_task.is_some() && !d.sched.active_task_detached)
			.map(|(id, _)| *id)
	}

	pub fn dirty_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries
			.iter()
			.filter(|(_, e)| e.slot.dirty)
			.map(|(id, _)| *id)
	}

	/// Returns true if any background task has completed its work.
	pub fn any_task_finished(&self) -> bool {
		self.collector.any_finished()
	}
}
