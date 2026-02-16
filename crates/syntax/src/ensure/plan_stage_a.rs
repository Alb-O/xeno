use super::*;

/// Stage-A planner: urgent viewport parse for uncovered or history-urgent repair.
pub(super) fn plan_stage_a(entry: &DocEntry, now: Instant, ctx: &EnsureViewport<'_>, metrics: &SyntaxMetrics) -> Option<LanePlan> {
	let base = &ctx.lang.base;
	let cfg = base.cfg;
	if base.tier != SyntaxTier::L
		|| base.hotness != SyntaxHotness::Visible
		|| entry.sched.viewport_urgent_active()
		|| entry.sched.lanes.viewport_urgent.in_cooldown(now)
	{
		return None;
	}

	let viewport_key = compute_viewport_key(ctx.viewport.start, cfg.viewport_window_max);
	let viewport_uncovered = entry.slot.full.is_none() && !entry.slot.viewport_cache.covers_range(&ctx.viewport);
	let history_stage_a_failed_for_doc_version = entry.slot.viewport_cache.map.get(&viewport_key).and_then(|ce| ce.stage_a_failed_for);
	let history_needs_eager_repair = entry.sched.last_edit_source == EditSource::History
		&& !slot_has_eager_exact_viewport_tree_coverage(&entry.slot, &ctx.viewport, base.doc_version)
		&& history_stage_a_failed_for_doc_version != Some(base.doc_version);
	tracing::trace!(
		target: "xeno_undo_trace",
		doc_id = ?base.doc_id,
		doc_version = base.doc_version,
		viewport = ?ctx.viewport,
		viewport_key = viewport_key.0,
		viewport_uncovered,
		history_needs_eager_repair,
		history_stage_a_failed_for_doc_version,
		last_edit_source = ?entry.sched.last_edit_source,
		"syntax.ensure.stage_a.decide"
	);

	if !(viewport_uncovered || history_needs_eager_repair) {
		return None;
	}

	let win_start = viewport_key.0.saturating_sub(cfg.viewport_lookbehind);
	let mut win_end = viewport_key.0.saturating_add(cfg.viewport_window_max).min(base.bytes_u32);
	win_end = win_end.max(ctx.viewport.end.saturating_add(cfg.viewport_lookahead).min(base.bytes_u32));
	let mut win_len = win_end.saturating_sub(win_start);
	if win_len > cfg.viewport_window_max {
		win_len = cfg.viewport_window_max;
		win_end = win_start.saturating_add(win_len).min(base.bytes_u32);
	}
	if win_end <= win_start {
		return None;
	}

	let history_urgent = base.tier == SyntaxTier::L && entry.sched.last_edit_source == EditSource::History;
	let injections = if history_urgent { InjectionPolicy::Eager } else { cfg.viewport_injections };
	let mut parse_timeout = metrics.derive_timeout(
		ctx.lang.language_id,
		base.tier,
		TaskClass::Viewport,
		injections,
		cfg.viewport_parse_timeout_min,
		cfg.viewport_parse_timeout_max,
	);
	if history_urgent {
		parse_timeout = parse_timeout.max(cfg.viewport_parse_timeout_max * 3);
	}

	Some(LanePlan {
		lane: PlanLane::StageA,
		spec: TaskSpec {
			doc_id: base.doc_id,
			epoch: entry.sched.epoch,
			doc_version: base.doc_version,
			lang_id: ctx.lang.language_id,
			opts_key: OptKey { injections },
			opts: SyntaxOptions { parse_timeout, injections },
			kind: TaskKind::ViewportParse {
				content: base.content.clone(),
				window: win_start..win_end,
			},
			loader: Arc::clone(base.loader),
			viewport_key: Some(viewport_key),
			viewport_lane: Some(scheduling::ViewportLane::Urgent),
		},
		privileged: true,
		apply: LaneApply::None,
	})
}
