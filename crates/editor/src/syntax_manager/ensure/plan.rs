use super::*;

/// Side-effect to apply after a successful spawn.
pub(super) enum PostSpawn {
	None,
	/// Mark Stage-B as attempted for this key/version and clear its cooldown latch.
	StageBMarkAttempt {
		key: ViewportKey,
		doc_version: u64,
	},
}

/// A single spawn request produced by the planning phase.
pub(super) struct SpawnReq {
	pub(super) spec: TaskSpec,
	pub(super) privileged: bool,
	pub(super) post: PostSpawn,
}

/// Work plan produced by `compute_plan`, consumed by `spawn_plan`.
#[derive(Default)]
pub(super) struct WorkPlan {
	pub(super) stage_a: Option<SpawnReq>,
	pub(super) stage_b: Option<SpawnReq>,
	pub(super) bg: Option<SpawnReq>,
}

#[derive(Clone, Copy)]
enum SpawnLane {
	StageA,
	StageB,
	Background,
}

/// Pure planning phase: decides what to spawn without mutating entry state.
pub(super) fn compute_plan(
	entry: &DocEntry,
	now: Instant,
	ctx: &EnsureSyntaxContext<'_>,
	d: &EnsureDerived,
	g: &GateState,
	metrics: &SyntaxMetrics,
) -> WorkPlan {
	let cfg = &d.cfg;
	let lang_id = ctx.language_id.unwrap();
	let mut plan = WorkPlan::default();

	// ── Stage-A: viewport urgent ─────────────────────────────────────
	let stage_a_eligible = d.tier == SyntaxTier::L
		&& ctx.hotness == SyntaxHotness::Visible
		&& d.viewport.is_some()
		&& !entry.sched.viewport_urgent_active()
		&& !entry.sched.lanes.viewport_urgent.in_cooldown(now);

	if stage_a_eligible {
		let viewport = d.viewport.as_ref().unwrap();
		let viewport_key = compute_viewport_key(viewport.start, cfg.viewport_window_max);
		let viewport_uncovered = entry.slot.full.is_none() && !entry.slot.viewport_cache.covers_range(viewport);
		let history_stage_a_failed_for_doc_version = entry.slot.viewport_cache.map.get(&viewport_key).and_then(|ce| ce.stage_a_failed_for);
		let history_needs_eager_repair = entry.sched.last_edit_source == EditSource::History
			&& !slot_has_eager_exact_viewport_tree_coverage(&entry.slot, viewport, ctx.doc_version)
			&& history_stage_a_failed_for_doc_version != Some(ctx.doc_version);
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			viewport = ?viewport,
			viewport_key = viewport_key.0,
			viewport_uncovered,
			history_needs_eager_repair,
			history_stage_a_failed_for_doc_version,
			last_edit_source = ?entry.sched.last_edit_source,
			"syntax.ensure.stage_a.decide"
		);

		if viewport_uncovered || history_needs_eager_repair {
			let win_start = viewport_key.0.saturating_sub(cfg.viewport_lookbehind);
			let mut win_end = viewport_key.0.saturating_add(cfg.viewport_window_max).min(d.bytes_u32);
			win_end = win_end.max(viewport.end.saturating_add(cfg.viewport_lookahead).min(d.bytes_u32));
			let mut win_len = win_end.saturating_sub(win_start);
			if win_len > cfg.viewport_window_max {
				win_len = cfg.viewport_window_max;
				win_end = win_start.saturating_add(win_len).min(d.bytes_u32);
			}

			if win_end > win_start {
				let history_urgent = d.tier == SyntaxTier::L && entry.sched.last_edit_source == EditSource::History;
				let injections = if history_urgent { InjectionPolicy::Eager } else { cfg.viewport_injections };
				let mut parse_timeout = metrics.derive_timeout(
					lang_id,
					d.tier,
					TaskClass::Viewport,
					injections,
					cfg.viewport_parse_timeout_min,
					cfg.viewport_parse_timeout_max,
				);
				if history_urgent {
					parse_timeout = parse_timeout.max(cfg.viewport_parse_timeout_max * 3);
				}

				plan.stage_a = Some(SpawnReq {
					spec: TaskSpec {
						doc_id: ctx.doc_id,
						epoch: entry.sched.epoch,
						doc_version: ctx.doc_version,
						lang_id,
						opts_key: OptKey { injections },
						opts: SyntaxOptions { parse_timeout, injections },
						kind: TaskKind::ViewportParse {
							content: ctx.content.clone(),
							window: win_start..win_end,
						},
						loader: Arc::clone(ctx.loader),
						viewport_key: Some(viewport_key),
						viewport_lane: Some(scheduling::ViewportLane::Urgent),
					},
					privileged: true,
					post: PostSpawn::None,
				});
			}
		}
	}

	// ── Stage-B: viewport enrich ─────────────────────────────────────
	// Stage-B must not plan if Stage-A is already active OR planned.
	let stage_b_eligible = d.tier == SyntaxTier::L
		&& ctx.hotness == SyntaxHotness::Visible
		&& d.viewport.is_some()
		&& !entry.sched.viewport_enrich_active()
		&& !entry.sched.viewport_urgent_active()
		&& plan.stage_a.is_none()
		&& cfg.viewport_stage_b_budget.is_some()
		&& g.viewport_stable_polls >= cfg.viewport_stage_b_min_stable_polls;

	if stage_b_eligible {
		let viewport = d.viewport.as_ref().unwrap();
		let k = entry
			.slot
			.viewport_cache
			.covering_key(viewport)
			.unwrap_or_else(|| compute_viewport_key(viewport.start, cfg.viewport_window_max));
		let cache_entry = entry.slot.viewport_cache.map.get(&k);
		let eager_covers = slot_has_stage_b_exact_viewport_coverage(&entry.slot, viewport, ctx.doc_version);
		let already_attempted = cache_entry.is_some_and(|ce| ce.attempted_b_for == Some(ctx.doc_version));
		let in_cooldown = cache_entry.is_some_and(|ce| ce.stage_b_cooldown_until.is_some_and(|until| now < until));
		tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			viewport = ?viewport,
			viewport_key = k.0,
			eager_covers,
			already_attempted,
			in_cooldown,
			viewport_stable_polls = g.viewport_stable_polls,
			"syntax.ensure.stage_b.decide"
		);

		if !eager_covers && !already_attempted && !in_cooldown {
			let budget = cfg.viewport_stage_b_budget.unwrap();
			let predicted = metrics.predict_duration(lang_id, d.tier, TaskClass::Viewport, InjectionPolicy::Eager);
			let within_budget = predicted.map(|p| p <= budget).unwrap_or(true);
			tracing::trace!(
				target: "xeno_undo_trace",
				doc_id = ?ctx.doc_id,
				doc_version = ctx.doc_version,
				budget_ms = budget.as_millis() as u64,
				predicted_ms = predicted.map(|p| p.as_millis() as u64),
				within_budget,
				"syntax.ensure.stage_b.budget"
			);

			if within_budget {
				let stage_b_win = cache_entry.and_then(|ce| ce.stage_a.as_ref().map(|sa| sa.coverage.clone()));
				let (win_start, win_end) = if let Some(range) = stage_b_win.as_ref() {
					(range.start, range.end)
				} else {
					let win_start = k.0.saturating_sub(cfg.viewport_lookbehind);
					let mut win_end = viewport.end.saturating_add(cfg.viewport_lookahead).min(d.bytes_u32);
					let mut win_len = win_end.saturating_sub(win_start);
					if win_len > cfg.viewport_window_max {
						win_len = cfg.viewport_window_max;
						win_end = win_start.saturating_add(win_len).min(d.bytes_u32);
					}
					(win_start, win_end)
				};

				if win_end > win_start {
					let injections = InjectionPolicy::Eager;
					let parse_timeout = metrics.derive_timeout(
						lang_id,
						d.tier,
						TaskClass::Viewport,
						injections,
						cfg.viewport_parse_timeout_min,
						cfg.viewport_parse_timeout_max,
					);

					plan.stage_b = Some(SpawnReq {
						spec: TaskSpec {
							doc_id: ctx.doc_id,
							epoch: entry.sched.epoch,
							doc_version: ctx.doc_version,
							lang_id,
							opts_key: OptKey { injections },
							opts: SyntaxOptions { parse_timeout, injections },
							kind: TaskKind::ViewportParse {
								content: ctx.content.clone(),
								window: win_start..win_end,
							},
							loader: Arc::clone(ctx.loader),
							viewport_key: Some(k),
							viewport_lane: Some(scheduling::ViewportLane::Enrich),
						},
						privileged: false,
						post: PostSpawn::StageBMarkAttempt {
							key: k,
							doc_version: ctx.doc_version,
						},
					});
				}
			} else {
				tracing::trace!(
					target: "xeno_undo_trace",
					doc_id = ?ctx.doc_id,
					doc_version = ctx.doc_version,
					budget_ms = budget.as_millis() as u64,
					predicted_ms = predicted.map(|p| p.as_millis() as u64),
					"syntax.ensure.stage_b.skipped_budget"
				);
			}
		}
	}

	// ── Background: full or incremental ──────────────────────────────
	let bg_needed = (entry.slot.dirty || entry.slot.full.is_none()) && !entry.sched.bg_active() && !entry.sched.lanes.bg.in_cooldown(now);

	if bg_needed {
		let incremental = match entry.slot.pending_incremental.as_ref() {
			Some(pending) if entry.slot.full.as_ref().is_some_and(|t| t.doc_version == pending.base_tree_doc_version) => Some(TaskKind::Incremental {
				base: entry.slot.full.as_ref().unwrap().syntax.clone(),
				old_rope: pending.old_rope.clone(),
				new_rope: ctx.content.clone(),
				composed: pending.composed.clone(),
			}),
			_ => None,
		};
		let kind = incremental.unwrap_or_else(|| TaskKind::FullParse { content: ctx.content.clone() });
		let class = kind.class();
		let injections = cfg.injections;
		let parse_timeout = metrics.derive_timeout(lang_id, d.tier, class, injections, cfg.parse_timeout_min, cfg.parse_timeout_max);

		plan.bg = Some(SpawnReq {
			spec: TaskSpec {
				doc_id: ctx.doc_id,
				epoch: entry.sched.epoch,
				doc_version: ctx.doc_version,
				lang_id,
				opts_key: d.opts_key,
				opts: SyntaxOptions { parse_timeout, injections },
				kind,
				loader: Arc::clone(ctx.loader),
				viewport_key: None,
				viewport_lane: None,
			},
			privileged: false,
			post: PostSpawn::None,
		});
	}

	plan
}

fn apply_spawn_success(entry: &mut DocEntry, lane: SpawnLane, task_id: TaskId, doc_version: u64, post: PostSpawn) {
	match lane {
		SpawnLane::StageA => {
			entry.sched.lanes.viewport_urgent.active = Some(task_id);
			entry.sched.lanes.viewport_urgent.requested_doc_version = doc_version;
		}
		SpawnLane::StageB => {
			entry.sched.lanes.viewport_enrich.active = Some(task_id);
			entry.sched.lanes.viewport_enrich.requested_doc_version = doc_version;
			debug_assert!(
				entry.sched.lanes.viewport_enrich.cooldown_until.is_none(),
				"viewport_enrich uses per-key cooldown, not lane-level"
			);
			if let PostSpawn::StageBMarkAttempt { key, doc_version } = post {
				let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
				ce.attempted_b_for = Some(doc_version);
				ce.stage_b_cooldown_until = None;
			}
		}
		SpawnLane::Background => {
			entry.sched.lanes.bg.active = Some(task_id);
			entry.sched.lanes.bg.requested_doc_version = doc_version;
			entry.sched.force_no_debounce = false;
		}
	}
}

fn trace_spawned(
	lane: SpawnLane,
	ctx: &EnsureSyntaxContext<'_>,
	task_id: TaskId,
	class: TaskClass,
	viewport_key: Option<ViewportKey>,
	injections: InjectionPolicy,
	parse_timeout: Duration,
) {
	match lane {
		SpawnLane::StageA => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			task_id = task_id.0,
			viewport_key = ?viewport_key.map(|k| k.0),
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.stage_a.spawned"
		),
		SpawnLane::StageB => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			task_id = task_id.0,
			viewport_key = ?viewport_key.map(|k| k.0),
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.stage_b.spawned"
		),
		SpawnLane::Background => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			task_id = task_id.0,
			?class,
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.background.spawned"
		),
	}
}

fn trace_spawn_rejected(
	lane: SpawnLane,
	ctx: &EnsureSyntaxContext<'_>,
	class: TaskClass,
	viewport_key: Option<ViewportKey>,
	injections: InjectionPolicy,
	parse_timeout: Duration,
) {
	match lane {
		SpawnLane::StageA => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			viewport_key = ?viewport_key.map(|k| k.0),
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.stage_a.spawn_rejected"
		),
		SpawnLane::StageB => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			viewport_key = ?viewport_key.map(|k| k.0),
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.stage_b.spawn_rejected"
		),
		SpawnLane::Background => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.doc_id,
			doc_version = ctx.doc_version,
			?class,
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.background.spawn_rejected"
		),
	}
}

fn spawn_lane(
	entry: &mut DocEntry,
	ctx: &EnsureSyntaxContext<'_>,
	lane: SpawnLane,
	req: SpawnReq,
	collector: &mut TaskCollector,
	permits: &Arc<Semaphore>,
	engine: &Arc<dyn SyntaxEngine>,
	mgr_cfg: &SyntaxManagerCfg,
) -> bool {
	let SpawnReq { spec, privileged, post } = req;
	let class = spec.kind.class();
	let viewport_key = spec.viewport_key;
	let injections = spec.opts.injections;
	let parse_timeout = spec.opts.parse_timeout;
	if let Some(task_id) = collector.spawn(Arc::clone(permits), Arc::clone(engine), spec, mgr_cfg.viewport_reserve, privileged) {
		apply_spawn_success(entry, lane, task_id, ctx.doc_version, post);
		trace_spawned(lane, ctx, task_id, class, viewport_key, injections, parse_timeout);
		true
	} else {
		trace_spawn_rejected(lane, ctx, class, viewport_key, injections, parse_timeout);
		false
	}
}

/// Spawns the work plan, applying side effects on success. Returns kicked_any.
pub(super) fn spawn_plan(
	entry: &mut DocEntry,
	ctx: &EnsureSyntaxContext<'_>,
	plan: WorkPlan,
	collector: &mut TaskCollector,
	permits: &Arc<Semaphore>,
	engine: &Arc<dyn SyntaxEngine>,
	mgr_cfg: &SyntaxManagerCfg,
) -> bool {
	let mut kicked_any = false;

	if let Some(req) = plan.stage_a {
		kicked_any |= spawn_lane(entry, ctx, SpawnLane::StageA, req, collector, permits, engine, mgr_cfg);
	}
	if let Some(req) = plan.stage_b {
		kicked_any |= spawn_lane(entry, ctx, SpawnLane::StageB, req, collector, permits, engine, mgr_cfg);
	}
	if let Some(req) = plan.bg {
		kicked_any |= spawn_lane(entry, ctx, SpawnLane::Background, req, collector, permits, engine, mgr_cfg);
	}

	kicked_any
}
