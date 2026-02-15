use std::ops::ControlFlow;

use super::*;

/// Language checks, gating, and scheduling state computation.
///
/// Returns `Continue(GateState)` if scheduling should proceed, or `Break(outcome)` for early exit.
///
/// Retention is handled by `sweep_retention` before the workset is built, not here.
/// This keeps a single owner for tree eviction and avoids surprising
/// "drop during visible ensure" edges.
pub(super) fn gate(entry: &mut DocEntry, now: Instant, ctx: &EnsureSyntaxContext<'_>, d: &EnsureDerived, was_updated: bool) -> Flow<GateState> {
	let cfg = d.cfg;
	let mut was_updated = was_updated;

	// Language check
	let Some(_lang_id) = ctx.language_id else {
		if entry.slot.has_any_tree() {
			entry.slot.drop_tree();
			SyntaxManager::mark_updated(&mut entry.slot);
			was_updated = true;
		}
		entry.slot.language_id = None;
		entry.slot.dirty = false;
		entry.sched.lanes.viewport_urgent.cooldown_until = None;
		entry.sched.lanes.viewport_enrich.cooldown_until = None;
		entry.sched.lanes.bg.cooldown_until = None;
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			updated = was_updated,
			"syntax.ensure.return.no_language"
		);
		return ControlFlow::Break(SyntaxPollOutcome {
			result: SyntaxPollResult::NoLanguage,
			updated: was_updated,
		});
	};

	// Work disabled gate
	if d.work_disabled {
		if entry.sched.any_active() {
			entry.sched.invalidate();
		}
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			updated = was_updated,
			"syntax.ensure.return.disabled"
		);
		return ControlFlow::Break(SyntaxPollOutcome {
			result: SyntaxPollResult::Disabled,
			updated: was_updated,
		});
	};

	// Viewport stability tracking
	let viewport_stable_polls = if let Some(vp) = &d.viewport {
		let focus_key = entry
			.slot
			.viewport_cache
			.covering_key(vp)
			.unwrap_or_else(|| compute_viewport_key(vp.start, cfg.viewport_window_max));
		entry.sched.note_viewport_focus(focus_key, ctx.doc_version)
	} else {
		0
	};

	// MRU touch
	if let Some(vp) = &d.viewport {
		if let Some(covering) = entry.slot.viewport_cache.covering_key(vp) {
			entry.slot.viewport_cache.touch(covering);
		} else {
			let key = compute_viewport_key(vp.start, cfg.viewport_window_max);
			if entry.slot.viewport_cache.map.contains_key(&key) {
				entry.slot.viewport_cache.touch(key);
			}
		}
	}

	// Enrichment desire
	let want_enrich = d.tier == SyntaxTier::L && ctx.hotness == SyntaxHotness::Visible && cfg.viewport_stage_b_budget.is_some() && d.viewport.is_some() && {
		let vp = d.viewport.as_ref().unwrap();
		!slot_has_stage_b_exact_viewport_coverage(&entry.slot, vp, ctx.doc_version)
	};

	let viewport_uncovered =
		d.tier == SyntaxTier::L && entry.slot.full.is_none() && d.viewport.as_ref().is_some_and(|vp| !entry.slot.viewport_cache.covers_range(vp));

	// Ready fast path
	if entry.slot.has_any_tree() && !entry.slot.dirty && !want_enrich && !viewport_uncovered {
		entry.sched.force_no_debounce = false;
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			updated = was_updated,
			"syntax.ensure.return.ready_fast_path"
		);
		return ControlFlow::Break(SyntaxPollOutcome {
			result: SyntaxPollResult::Ready,
			updated: was_updated,
		});
	}

	// Debounce gate
	if entry.slot.has_any_tree() && !entry.sched.force_no_debounce && now.duration_since(entry.sched.last_edit_at) < cfg.debounce {
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			updated = was_updated,
			force_no_debounce = entry.sched.force_no_debounce,
			"syntax.ensure.return.pending_debounce"
		);
		return ControlFlow::Break(SyntaxPollOutcome {
			result: SyntaxPollResult::Pending,
			updated: was_updated,
		});
	}

	ControlFlow::Continue(GateState {
		viewport_stable_polls,
		viewport_uncovered,
	})
}
