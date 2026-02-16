use super::*;

#[path = "plan_bg.rs"]
mod plan_bg;
#[path = "plan_stage_a.rs"]
mod plan_stage_a;
#[path = "plan_stage_b.rs"]
mod plan_stage_b;

use plan_bg::plan_bg;
use plan_stage_a::plan_stage_a;
use plan_stage_b::plan_stage_b;

/// Side-effect to apply after a successful lane spawn.
pub(super) enum LaneApply {
	None,
	/// Mark Stage-B as attempted for this key/version and clear its cooldown latch.
	StageBMarkAttempt {
		key: ViewportKey,
		doc_version: u64,
	},
}

/// Logical scheduling lane used by planner and spawn applicator.
#[derive(Clone, Copy)]
pub(super) enum PlanLane {
	StageA,
	StageB,
	Background,
}

/// Spawn request produced by one lane planner.
pub(super) struct LanePlan {
	pub(super) lane: PlanLane,
	pub(super) spec: TaskSpec,
	pub(super) privileged: bool,
	pub(super) apply: LaneApply,
}

/// Composite work plan produced by `compute_plan` and consumed by `spawn_plan`.
#[derive(Default)]
pub(super) struct PlanSet {
	pub(super) stage_a: Option<LanePlan>,
	pub(super) stage_b: Option<LanePlan>,
	pub(super) bg: Option<LanePlan>,
}

impl PlanSet {
	pub(super) fn planned_any(&self) -> bool {
		self.stage_a.is_some() || self.stage_b.is_some() || self.bg.is_some()
	}
}

/// Summary from the spawn phase.
#[derive(Clone, Copy, Default)]
pub(super) struct PlanSummary {
	pub(super) planned_any: bool,
	pub(super) kicked_any: bool,
}

/// Pure planning phase: decides what to spawn without mutating entry state.
pub(super) fn compute_plan(entry: &DocEntry, now: Instant, ctx: &EnsureLang<'_>, g: &GateState, metrics: &SyntaxMetrics) -> PlanSet {
	let viewport = ctx.viewport();
	let stage_a = viewport.as_ref().and_then(|vp| plan_stage_a(entry, now, vp, metrics));
	let stage_b = viewport.as_ref().and_then(|vp| {
		if stage_a.is_some() || entry.sched.viewport_urgent_active() {
			None
		} else {
			plan_stage_b(entry, now, vp, g, metrics)
		}
	});
	let bg = plan_bg(entry, now, ctx, metrics);

	PlanSet { stage_a, stage_b, bg }
}

fn apply_spawn_success(entry: &mut DocEntry, lane: PlanLane, task_id: TaskId, doc_version: u64, apply: LaneApply) {
	match lane {
		PlanLane::StageA => {
			entry.sched.lanes.viewport_urgent.active = Some(task_id);
			entry.sched.lanes.viewport_urgent.requested_doc_version = doc_version;
		}
		PlanLane::StageB => {
			entry.sched.lanes.viewport_enrich.active = Some(task_id);
			entry.sched.lanes.viewport_enrich.requested_doc_version = doc_version;
			debug_assert!(
				entry.sched.lanes.viewport_enrich.cooldown_until.is_none(),
				"viewport_enrich uses per-key cooldown, not lane-level"
			);
			if let LaneApply::StageBMarkAttempt { key, doc_version } = apply {
				let ce = entry.slot.viewport_cache.get_mut_or_insert(key);
				ce.attempted_b_for = Some(doc_version);
				ce.stage_b_cooldown_until = None;
			}
		}
		PlanLane::Background => {
			entry.sched.lanes.bg.active = Some(task_id);
			entry.sched.lanes.bg.requested_doc_version = doc_version;
			entry.sched.force_no_debounce = false;
		}
	}
}

fn trace_spawned(
	ctx: &EnsureLang<'_>,
	lane: PlanLane,
	task_id: TaskId,
	class: TaskClass,
	viewport_key: Option<ViewportKey>,
	injections: InjectionPolicy,
	parse_timeout: Duration,
) {
	match lane {
		PlanLane::StageA => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.base.doc_id,
			doc_version = ctx.base.doc_version,
			task_id = task_id.0,
			viewport_key = ?viewport_key.map(|k| k.0),
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.stage_a.spawned"
		),
		PlanLane::StageB => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.base.doc_id,
			doc_version = ctx.base.doc_version,
			task_id = task_id.0,
			viewport_key = ?viewport_key.map(|k| k.0),
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.stage_b.spawned"
		),
		PlanLane::Background => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.base.doc_id,
			doc_version = ctx.base.doc_version,
			task_id = task_id.0,
			?class,
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.background.spawned"
		),
	}
}

fn trace_spawn_rejected(
	ctx: &EnsureLang<'_>,
	lane: PlanLane,
	class: TaskClass,
	viewport_key: Option<ViewportKey>,
	injections: InjectionPolicy,
	parse_timeout: Duration,
) {
	match lane {
		PlanLane::StageA => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.base.doc_id,
			doc_version = ctx.base.doc_version,
			viewport_key = ?viewport_key.map(|k| k.0),
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.stage_a.spawn_rejected"
		),
		PlanLane::StageB => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.base.doc_id,
			doc_version = ctx.base.doc_version,
			viewport_key = ?viewport_key.map(|k| k.0),
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.stage_b.spawn_rejected"
		),
		PlanLane::Background => tracing::trace!(
			target: "xeno_undo_trace",
			doc_id = ?ctx.base.doc_id,
			doc_version = ctx.base.doc_version,
			?class,
			injections = ?injections,
			parse_timeout_ms = parse_timeout.as_millis() as u64,
			"syntax.ensure.background.spawn_rejected"
		),
	}
}

fn spawn_lane(
	entry: &mut DocEntry,
	ctx: &EnsureLang<'_>,
	req: LanePlan,
	collector: &mut TaskCollector,
	permits: &Arc<Semaphore>,
	engine: &Arc<dyn SyntaxEngine>,
	mgr_cfg: &SyntaxManagerCfg,
) -> bool {
	let LanePlan { lane, spec, privileged, apply } = req;
	let class = spec.kind.class();
	let viewport_key = spec.viewport_key;
	let injections = spec.opts.injections;
	let parse_timeout = spec.opts.parse_timeout;
	if let Some(task_id) = collector.spawn(Arc::clone(permits), Arc::clone(engine), spec, mgr_cfg.viewport_reserve, privileged) {
		apply_spawn_success(entry, lane, task_id, ctx.base.doc_version, apply);
		trace_spawned(ctx, lane, task_id, class, viewport_key, injections, parse_timeout);
		true
	} else {
		trace_spawn_rejected(ctx, lane, class, viewport_key, injections, parse_timeout);
		false
	}
}

/// Spawns the work plan, applying side effects on success.
pub(super) fn spawn_plan(
	entry: &mut DocEntry,
	ctx: &EnsureLang<'_>,
	plan: PlanSet,
	collector: &mut TaskCollector,
	permits: &Arc<Semaphore>,
	engine: &Arc<dyn SyntaxEngine>,
	mgr_cfg: &SyntaxManagerCfg,
) -> PlanSummary {
	let planned_any = plan.planned_any();
	let mut kicked_any = false;

	if let Some(req) = plan.stage_a {
		kicked_any |= spawn_lane(entry, ctx, req, collector, permits, engine, mgr_cfg);
	}
	if let Some(req) = plan.stage_b {
		kicked_any |= spawn_lane(entry, ctx, req, collector, permits, engine, mgr_cfg);
	}
	if let Some(req) = plan.bg {
		kicked_any |= spawn_lane(entry, ctx, req, collector, permits, engine, mgr_cfg);
	}

	PlanSummary { planned_any, kicked_any }
}
