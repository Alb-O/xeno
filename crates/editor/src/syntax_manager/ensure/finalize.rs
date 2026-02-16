use super::*;

#[derive(Clone, Copy)]
struct DesiredWorkFlags {
	dirty_or_missing_full: bool,
	viewport_uncovered: bool,
	planned_any: bool,
}

impl DesiredWorkFlags {
	fn any(self) -> bool {
		self.dirty_or_missing_full || self.viewport_uncovered || self.planned_any
	}
}

/// Computes the final poll result.
///
/// `planned_any` reflects whether `compute_plan` produced any spawn requests,
/// replacing the old `want_enrich` heuristic so `Pending` only fires when work
/// was actually schedulable (not blocked by attempted latch, budget, or cooldown).
pub(super) fn finalize(entry: &DocEntry, now: Instant, ctx: &EnsureLang<'_>, g: &GateState, was_updated: bool, summary: PlanSummary) -> SyntaxPollOutcome {
	if summary.kicked_any {
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.base.doc_id,
			doc_version = ctx.base.doc_version,
			updated = was_updated,
			"syntax.ensure.return.kicked"
		);
		return SyntaxPollOutcome {
			result: SyntaxPollResult::Kicked,
			updated: was_updated,
		};
	}

	let desired = DesiredWorkFlags {
		dirty_or_missing_full: entry.slot.dirty || entry.slot.full.is_none(),
		viewport_uncovered: g.viewport_uncovered,
		planned_any: summary.planned_any,
	};
	let desired_work = desired.any();
	let any_lane_cooling = entry.sched.lanes.viewport_urgent.in_cooldown(now) || entry.sched.lanes.bg.in_cooldown(now);

	if entry.sched.any_active() || desired_work {
		if !entry.sched.any_active() && any_lane_cooling {
			tracing::trace!(
				target: "xeno_undo_trace",
				doc_id = ?ctx.base.doc_id,
				doc_version = ctx.base.doc_version,
				updated = was_updated,
				"syntax.ensure.return.cooling_down"
			);
			SyntaxPollOutcome {
				result: SyntaxPollResult::CoolingDown,
				updated: was_updated,
			}
		} else {
			tracing::trace!(
				target: "xeno_undo_trace",
				doc_id = ?ctx.base.doc_id,
				doc_version = ctx.base.doc_version,
				updated = was_updated,
				desired_work,
				active = entry.sched.any_active(),
				"syntax.ensure.return.pending"
			);
			SyntaxPollOutcome {
				result: SyntaxPollResult::Pending,
				updated: was_updated,
			}
		}
	} else {
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.base.doc_id,
			doc_version = ctx.base.doc_version,
			updated = was_updated,
			desired_work,
			active = entry.sched.any_active(),
			"syntax.ensure.return.ready"
		);
		SyntaxPollOutcome {
			result: SyntaxPollResult::Ready,
			updated: was_updated,
		}
	}
}
