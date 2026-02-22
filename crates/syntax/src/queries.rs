use xeno_language::{InjectionPolicy, Syntax};

use super::*;

impl SyntaxManager {
	/// Returns true if any syntax tree is installed for the document.
	pub fn has_syntax(&self, doc_id: DocumentId) -> bool {
		self.entries.get(&doc_id).is_some_and(|e| e.slot.has_any_tree())
	}

	pub fn is_dirty(&self, doc_id: DocumentId) -> bool {
		self.entries.get(&doc_id).map(|e| e.slot.dirty).unwrap_or(false)
	}

	/// Returns a reference to the full tree if installed, falling back to any cached viewport tree.
	///
	/// Prefer [`Self::syntax_for_viewport`] for rendering, which selects the best
	/// available tree for a given viewport range.
	pub fn syntax_for_doc(&self, doc_id: DocumentId) -> Option<&Syntax> {
		let slot = &self.entries.get(&doc_id)?.slot;
		if let Some(ref full) = slot.full {
			return Some(&full.syntax);
		}
		// Fall back to best viewport tree from cache (MRU order, stage_b preferred)
		for key in slot.viewport_cache.iter_keys_mru() {
			let Some(entry) = slot.viewport_cache.map.get(&key) else { continue };
			if let Some(ref t) = entry.stage_b {
				return Some(&t.syntax);
			}
		}
		for key in slot.viewport_cache.iter_keys_mru() {
			let Some(entry) = slot.viewport_cache.map.get(&key) else { continue };
			if let Some(ref t) = entry.stage_a {
				return Some(&t.syntax);
			}
		}
		None
	}

	/// Selects the best available syntax tree for rendering a viewport range.
	///
	/// Uses a scoring comparator over all available candidates. Among candidates
	/// that overlap the viewport, prefer (in descending priority):
	/// 1. Exact doc_version match
	/// 2. Injection-eager trees (richer highlighting)
	/// 3. Full trees (wider coverage)
	/// 4. Higher tree_doc_version (more recent)
	///
	/// If no candidate overlaps, falls back to the best non-overlapping candidate.
	pub fn syntax_for_viewport(&self, doc_id: DocumentId, doc_version: u64, viewport: std::ops::Range<u32>) -> Option<SyntaxSelection<'_>> {
		let slot = &self.entries.get(&doc_id)?.slot;

		/// Scoring key for selection comparison (higher is better).
		///
		/// Full trees rank above viewport trees at the same version because
		/// they have complete structural context (e.g. a block comment
		/// spanning thousands of lines is only visible to the full parse).
		fn score(sel: &SyntaxSelection<'_>, doc_version: u64) -> (bool, bool, bool, u64) {
			let exact = sel.tree_doc_version == doc_version;
			let is_full = sel.coverage.is_none();
			let eager = sel.syntax.opts().injections == InjectionPolicy::Eager;
			(exact, is_full, eager, sel.tree_doc_version)
		}

		fn overlaps(coverage: &Option<std::ops::Range<u32>>, viewport: &std::ops::Range<u32>) -> bool {
			match coverage {
				None => true, // full tree always overlaps
				Some(c) => viewport.start < c.end && viewport.end > c.start,
			}
		}

		type Score = (bool, bool, bool, u64);
		let mut best_overlapping: Option<(SyntaxSelection<'_>, Score)> = None;
		let mut best_any: Option<(SyntaxSelection<'_>, Score)> = None;
		let mut candidate_count = 0usize;
		let mut overlapping_candidate_count = 0usize;

		macro_rules! consider {
			($sel:expr) => {{
				let sel = $sel;
				candidate_count += 1;
				let s = score(&sel, doc_version);
				let ovl = overlaps(&sel.coverage, &viewport);
				if ovl {
					overlapping_candidate_count += 1;
					if best_overlapping.as_ref().map_or(true, |(_, prev)| s > *prev) {
						best_overlapping = Some((sel, s));
					}
				} else if best_any.as_ref().map_or(true, |(_, prev)| s > *prev) {
					best_any = Some((sel, s));
				}
			}};
		}

		// Full tree candidate
		if let Some(ref s) = slot.full {
			consider!(SyntaxSelection {
				syntax: &s.syntax,
				tree_id: s.tree_id,
				tree_doc_version: s.doc_version,
				coverage: None,
			});
		}

		// Viewport cache candidates (MRU order for stable tie-breaking)
		for key in slot.viewport_cache.iter_keys_mru() {
			let Some(entry) = slot.viewport_cache.map.get(&key) else { continue };
			if let Some(ref t) = entry.stage_b {
				consider!(SyntaxSelection {
					syntax: &t.syntax,
					tree_id: t.tree_id,
					tree_doc_version: t.doc_version,
					coverage: Some(t.coverage.clone()),
				});
			}
			if let Some(ref t) = entry.stage_a {
				consider!(SyntaxSelection {
					syntax: &t.syntax,
					tree_id: t.tree_id,
					tree_doc_version: t.doc_version,
					coverage: Some(t.coverage.clone()),
				});
			}
		}

		let selection = best_overlapping.or(best_any).map(|(sel, _)| sel);
		match selection.as_ref() {
			Some(sel) => {
				let (cov_start, cov_end) = sel.coverage.as_ref().map_or((None, None), |c| (Some(c.start), Some(c.end)));
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version,
					viewport_start = viewport.start,
					viewport_end = viewport.end,
					candidate_count,
					overlapping_candidate_count,
					selected_tree_id = sel.tree_id,
					selected_tree_doc_version = sel.tree_doc_version,
					selected_is_full = sel.coverage.is_none(),
					selected_coverage_start = cov_start,
					selected_coverage_end = cov_end,
					selected_injections = ?sel.syntax.opts().injections,
					"syntax.query.syntax_for_viewport.selected"
				);
			}
			None => {
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version,
					viewport_start = viewport.start,
					viewport_end = viewport.end,
					candidate_count,
					overlapping_candidate_count,
					"syntax.query.syntax_for_viewport.none"
				);
			}
		}
		selection
	}

	/// Returns the document-global change counter for highlight cache invalidation.
	pub fn syntax_version(&self, doc_id: DocumentId) -> u64 {
		self.entries.get(&doc_id).map(|e| e.slot.change_id).unwrap_or(0)
	}

	/// Returns the document version that the installed full tree corresponds to.
	#[cfg(test)]
	pub(crate) fn syntax_doc_version(&self, doc_id: DocumentId) -> Option<u64> {
		let slot = &self.entries.get(&doc_id)?.slot;
		slot.full.as_ref().map(|t| t.doc_version).or(slot.viewport_cache.best_doc_version())
	}

	/// Returns projection context for mapping stale tree highlights onto current text.
	///
	/// Returns `None` when tree and target versions already match, or when no
	/// aligned pending window exists.
	#[cfg(test)]
	pub(crate) fn highlight_projection_ctx(&self, doc_id: DocumentId, doc_version: u64) -> Option<HighlightProjectionCtx<'_>> {
		let tree_doc_version = self.syntax_doc_version_internal(doc_id)?;
		self.highlight_projection_ctx_for(doc_id, tree_doc_version, doc_version)
	}

	/// Returns projection context for a specific tree version mapped to the target doc version.
	pub fn highlight_projection_ctx_for(&self, doc_id: DocumentId, tree_doc_version: u64, target_doc_version: u64) -> Option<HighlightProjectionCtx<'_>> {
		if tree_doc_version == target_doc_version {
			tracing::trace!(
				target: "xeno_undo_trace",
				?doc_id,
				tree_doc_version,
				target_doc_version,
				result = "none_already_aligned",
				"syntax.query.highlight_projection_ctx_for"
			);
			return None;
		}

		let entry = self.entries.get(&doc_id)?;
		let pending = entry.slot.pending_incremental.as_ref()?;
		if pending.base_tree_doc_version != tree_doc_version {
			tracing::trace!(
				target: "xeno_undo_trace",
				?doc_id,
				tree_doc_version,
				target_doc_version,
				pending_base_tree_doc_version = pending.base_tree_doc_version,
				result = "none_base_mismatch",
				"syntax.query.highlight_projection_ctx_for"
			);
			return None;
		}

		tracing::trace!(
			target: "xeno_undo_trace",
			?doc_id,
			tree_doc_version,
			target_doc_version,
			pending_base_tree_doc_version = pending.base_tree_doc_version,
			composed_op_count = pending.composed.changes().len(),
			result = "some",
			"syntax.query.highlight_projection_ctx_for"
		);
		Some(HighlightProjectionCtx {
			tree_doc_version,
			target_doc_version,
			base_rope: &pending.old_rope,
			composed_changes: &pending.composed,
		})
	}

	/// Internal helper: best available tree doc version.
	#[cfg(test)]
	fn syntax_doc_version_internal(&self, doc_id: DocumentId) -> Option<u64> {
		self.entries.get(&doc_id)?.slot.best_doc_version()
	}

	#[cfg(test)]
	pub(crate) fn has_inflight_viewport(&self, doc_id: DocumentId) -> bool {
		self.entries.get(&doc_id).is_some_and(|e| e.sched.viewport_any_active())
	}

	#[cfg(test)]
	pub(crate) fn has_inflight_viewport_urgent(&self, doc_id: DocumentId) -> bool {
		self.entries.get(&doc_id).is_some_and(|e| e.sched.viewport_urgent_active())
	}

	#[cfg(test)]
	pub(crate) fn has_inflight_viewport_enrich(&self, doc_id: DocumentId) -> bool {
		self.entries.get(&doc_id).is_some_and(|e| e.sched.viewport_enrich_active())
	}

	#[cfg(test)]
	pub(crate) fn has_inflight_bg(&self, doc_id: DocumentId) -> bool {
		self.entries.get(&doc_id).is_some_and(|e| e.sched.bg_active())
	}

	pub fn has_pending(&self, doc_id: DocumentId) -> bool {
		self.entries.get(&doc_id).is_some_and(|d| d.sched.any_active())
	}

	pub fn viewport_visible_span_cap_for_bytes(&self, bytes: usize) -> u32 {
		let tier = self.policy.tier_for_bytes(bytes);
		self.policy.cfg(tier).viewport_visible_span_cap
	}

	pub fn pending_count(&self) -> usize {
		self.entries.values().filter(|d| d.sched.any_active()).count()
	}

	pub fn pending_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries.iter().filter(|(_, d)| d.sched.any_active()).map(|(id, _)| *id)
	}

	pub fn dirty_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries.iter().filter(|(_, e)| e.slot.dirty).map(|(id, _)| *id)
	}

	/// Documents with unprocessed completed tasks in their queue.
	///
	/// These must be included in the render-frame workset so completions get
	/// installed/discarded even for docs that are no longer visible or dirty.
	pub fn docs_with_completed(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries.iter().filter(|(_, e)| !e.sched.completed.is_empty()).map(|(id, _)| *id)
	}

	/// Returns true if any background task has completed its work.
	pub fn any_task_finished(&self) -> bool {
		self.collector.any_finished()
	}

	/// Returns a snapshot of per-document internal state for test introspection.
	#[cfg(test)]
	pub(crate) fn debug_doc_state(&self, doc_id: DocumentId) -> Option<DebugDocState> {
		let entry = self.entries.get(&doc_id)?;
		Some(DebugDocState {
			dirty: entry.slot.dirty,
			sync_bootstrap_attempted: entry.slot.sync_bootstrap_attempted,
			full_doc_version: entry.slot.full.as_ref().map(|t| t.doc_version),
			pending_base_version: entry.slot.pending_incremental.as_ref().map(|p| p.base_tree_doc_version),
			bg_inflight: entry.sched.bg_active(),
			viewport_urgent_inflight: entry.sched.viewport_urgent_active(),
			viewport_enrich_inflight: entry.sched.viewport_enrich_active(),
			has_completed: !entry.sched.completed.is_empty(),
			full_tree_id: entry.slot.full.as_ref().map(|t| t.tree_id),
		})
	}
}

/// Test-only snapshot of per-document syntax state.
#[cfg(test)]
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct DebugDocState {
	pub dirty: bool,
	pub sync_bootstrap_attempted: bool,
	pub full_doc_version: Option<u64>,
	pub pending_base_version: Option<u64>,
	pub bg_inflight: bool,
	pub viewport_urgent_inflight: bool,
	pub viewport_enrich_inflight: bool,
	pub has_completed: bool,
	pub full_tree_id: Option<u64>,
}
