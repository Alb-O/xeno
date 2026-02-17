use super::*;

/// Stage-B planner: eager viewport enrichment for stable focus windows.
pub(super) fn plan_stage_b(entry: &DocEntry, now: Instant, ctx: &EnsureViewport<'_>, g: &GateState, metrics: &SyntaxMetrics) -> Option<LanePlan> {
	let base = &ctx.lang.base;
	let cfg = base.cfg;
	let budget = cfg.viewport_stage_b_budget?;
	if base.tier != SyntaxTier::L
		|| base.hotness != SyntaxHotness::Visible
		|| entry.sched.viewport_enrich_active()
		|| entry.sched.viewport_urgent_active()
		|| g.viewport_stable_polls < cfg.viewport_stage_b_min_stable_polls
	{
		return None;
	}

	let key = entry
		.slot
		.viewport_cache
		.covering_key(&ctx.viewport)
		.unwrap_or_else(|| compute_viewport_key(ctx.viewport.start, cfg.viewport_window_max));
	let cache_entry = entry.slot.viewport_cache.map.get(&key);
	let eager_covers = slot_has_stage_b_exact_viewport_coverage(&entry.slot, &ctx.viewport, base.doc_version);
	let already_attempted = cache_entry.is_some_and(|ce| ce.attempted_b_for == Some(base.doc_version));
	let in_cooldown = cache_entry.is_some_and(|ce| ce.stage_b_cooldown_until.is_some_and(|until| now < until));
	tracing::trace!(
		target: "xeno_undo_trace",
		doc_id = ?base.doc_id,
		doc_version = base.doc_version,
		viewport = ?ctx.viewport,
		viewport_key = key.0,
		eager_covers,
		already_attempted,
		in_cooldown,
		viewport_stable_polls = g.viewport_stable_polls,
		"syntax.ensure.stage_b.decide"
	);

	if eager_covers || already_attempted || in_cooldown {
		return None;
	}

	let predicted = metrics.predict_duration(ctx.lang.language_id, base.tier, TaskClass::Viewport, InjectionPolicy::Eager);
	let within_budget = predicted.map(|p| p <= budget).unwrap_or(true);
	tracing::trace!(
		target: "xeno_undo_trace",
		doc_id = ?base.doc_id,
		doc_version = base.doc_version,
		budget_ms = budget.as_millis() as u64,
		predicted_ms = predicted.map(|p| p.as_millis() as u64),
		within_budget,
		"syntax.ensure.stage_b.budget"
	);
	if !within_budget {
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?base.doc_id,
			doc_version = base.doc_version,
			budget_ms = budget.as_millis() as u64,
			predicted_ms = predicted.map(|p| p.as_millis() as u64),
			"syntax.ensure.stage_b.skipped_budget"
		);
		return None;
	}

	let stage_b_win = cache_entry.and_then(|ce| ce.stage_a.as_ref().map(|sa| sa.coverage.clone()));
	let (win_start, win_end) = if let Some(range) = stage_b_win {
		(range.start, range.end)
	} else {
		let win_start = key.0.saturating_sub(cfg.viewport_lookbehind);
		let mut win_end = ctx.viewport.end.saturating_add(cfg.viewport_lookahead).min(base.bytes_u32);
		let mut win_len = win_end.saturating_sub(win_start);
		if win_len > cfg.viewport_window_max {
			win_len = cfg.viewport_window_max;
			win_end = win_start.saturating_add(win_len).min(base.bytes_u32);
		}
		(win_start, win_end)
	};
	if win_end <= win_start {
		return None;
	}

	let injections = InjectionPolicy::Eager;
	let parse_timeout = metrics.derive_timeout(
		ctx.lang.language_id,
		base.tier,
		TaskClass::Viewport,
		injections,
		cfg.viewport_parse_timeout_min,
		cfg.viewport_parse_timeout_max,
	);

	Some(LanePlan {
		lane: PlanLane::StageB,
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
			viewport_key: Some(key),
			viewport_lane: Some(scheduling::ViewportLane::Enrich),
		},
		privileged: false,
		apply: LaneApply::StageBMarkAttempt {
			key,
			doc_version: base.doc_version,
		},
	})
}
