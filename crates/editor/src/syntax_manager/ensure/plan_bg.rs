use super::*;

/// Background planner: full or incremental parse catch-up.
pub(super) fn plan_bg(entry: &DocEntry, now: Instant, ctx: &EnsureLang<'_>, metrics: &SyntaxMetrics) -> Option<LanePlan> {
	let cfg = ctx.base.cfg;
	let bg_needed = (entry.slot.dirty || entry.slot.full.is_none()) && !entry.sched.bg_active() && !entry.sched.lanes.bg.in_cooldown(now);
	if !bg_needed {
		return None;
	}

	let incremental = match (&entry.slot.pending_incremental, &entry.slot.full) {
		(Some(pending), Some(full)) if full.doc_version == pending.base_tree_doc_version => Some(TaskKind::Incremental {
			base: full.syntax.clone(),
			old_rope: pending.old_rope.clone(),
			new_rope: ctx.base.content.clone(),
			composed: pending.composed.clone(),
		}),
		_ => None,
	};
	let kind = incremental.unwrap_or_else(|| TaskKind::FullParse {
		content: ctx.base.content.clone(),
	});
	let class = kind.class();
	let injections = cfg.injections;
	let parse_timeout = metrics.derive_timeout(ctx.language_id, ctx.base.tier, class, injections, cfg.parse_timeout_min, cfg.parse_timeout_max);

	Some(LanePlan {
		lane: PlanLane::Background,
		spec: TaskSpec {
			doc_id: ctx.base.doc_id,
			epoch: entry.sched.epoch,
			doc_version: ctx.base.doc_version,
			lang_id: ctx.language_id,
			opts_key: ctx.base.opts_key,
			opts: SyntaxOptions { parse_timeout, injections },
			kind,
			loader: Arc::clone(ctx.base.loader),
			viewport_key: None,
			viewport_lane: None,
		},
		privileged: false,
		apply: LaneApply::None,
	})
}
