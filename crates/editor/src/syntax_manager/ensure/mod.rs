//! Ensure-syntax pipeline orchestration.
//!
//! Implements the phased `ensure_syntax` flow (derive, normalize, install,
//! gate, bootstrap, plan, spawn, finalize) and shared helpers used by the
//! per-phase modules.
//!
//! The install phase follows an explicit evaluate/apply split: it evaluates
//! completion policy without mutating state, then applies the resulting action.

use std::ops::ControlFlow;

use super::*;

mod bootstrap;
mod derive;
mod finalize;
mod gate;
mod install;
mod normalize;
mod plan;

use bootstrap::sync_bootstrap;
use derive::derive;
use finalize::finalize;
use gate::gate;
use install::install_completions;
use normalize::normalize;
use plan::{PlanSummary, compute_plan, spawn_plan};

/// Early-exit flow type for ensure phases.
type Flow<T> = ControlFlow<SyntaxPollOutcome, T>;

/// Derived policy/input state computed once at the start of ensure.
#[derive(Clone)]
pub(super) struct EnsureBase<'a> {
	pub(super) doc_id: DocumentId,
	pub(super) doc_version: u64,
	pub(super) language_id: Option<xeno_language::LanguageId>,
	pub(super) content: &'a Rope,
	pub(super) hotness: SyntaxHotness,
	pub(super) loader: &'a Arc<LanguageLoader>,
	pub(super) bytes: usize,
	pub(super) bytes_u32: u32,
	pub(super) tier: SyntaxTier,
	pub(super) cfg: TierCfg,
	pub(super) opts_key: OptKey,
	pub(super) viewport: Option<std::ops::Range<u32>>,
	pub(super) work_disabled: bool,
}

impl<'a> EnsureBase<'a> {
	/// Converts to a language-ready context after gate validation.
	pub(super) fn into_lang(self, language_id: xeno_language::LanguageId) -> EnsureLang<'a> {
		EnsureLang { base: self, language_id }
	}
}

/// Context that guarantees language availability and work-enabled state.
#[derive(Clone)]
pub(super) struct EnsureLang<'a> {
	pub(super) base: EnsureBase<'a>,
	pub(super) language_id: xeno_language::LanguageId,
}

impl<'a> EnsureLang<'a> {
	/// Returns a viewport-ready context when a normalized viewport is present.
	pub(super) fn viewport(&self) -> Option<EnsureViewport<'a>> {
		self.base.viewport.clone().map(|viewport| EnsureViewport { lang: self.clone(), viewport })
	}
}

/// Context that guarantees viewport-dependent scheduling prerequisites.
#[derive(Clone)]
pub(super) struct EnsureViewport<'a> {
	pub(super) lang: EnsureLang<'a>,
	pub(super) viewport: std::ops::Range<u32>,
}

/// State computed during gating that feeds into scheduling.
#[derive(Default)]
pub(super) struct GateState {
	pub(super) viewport_stable_polls: u8,
	pub(super) viewport_uncovered: bool,
}

/// Gate output once language/work-disabled checks have passed.
pub(super) struct GateOutput<'a> {
	pub(super) ctx: EnsureLang<'a>,
	pub(super) state: GateState,
}

/// Computes an aligned viewport key for cache reuse.
pub(super) fn compute_viewport_key(viewport_start: u32, window_max: u32) -> ViewportKey {
	let stride = (window_max / 2).max(4096);
	let anchor = (viewport_start / stride) * stride;
	ViewportKey(anchor)
}

pub(super) fn slot_has_eager_exact_viewport_tree_coverage(slot: &SyntaxSlot, viewport: &std::ops::Range<u32>, doc_version: u64) -> bool {
	let Some(key) = slot.viewport_cache.covering_key(viewport) else {
		return false;
	};
	let Some(cache_entry) = slot.viewport_cache.map.get(&key) else {
		return false;
	};
	let exact_covers = |t: &ViewportTree| t.doc_version == doc_version && t.coverage.start <= viewport.start && t.coverage.end >= viewport.end;
	cache_entry.stage_b.as_ref().is_some_and(&exact_covers)
		|| cache_entry
			.stage_a
			.as_ref()
			.is_some_and(|t| t.syntax.opts().injections == InjectionPolicy::Eager && exact_covers(t))
}

pub(super) fn slot_has_stage_b_exact_viewport_coverage(slot: &SyntaxSlot, viewport: &std::ops::Range<u32>, doc_version: u64) -> bool {
	let Some(key) = slot.viewport_cache.covering_key(viewport) else {
		return false;
	};
	let Some(cache_entry) = slot.viewport_cache.map.get(&key) else {
		return false;
	};
	cache_entry
		.stage_b
		.as_ref()
		.is_some_and(|t| t.doc_version == doc_version && t.coverage.start <= viewport.start && t.coverage.end >= viewport.end)
}

impl SyntaxManager {
	/// Invariant enforcement: Polls or kicks background syntax parsing for a document.
	pub fn ensure_syntax(&mut self, ctx: EnsureSyntaxContext<'_>) -> SyntaxPollOutcome {
		self.ensure_syntax_at(Instant::now(), ctx)
	}

	/// Clock-injectable inner implementation of [`Self::ensure_syntax`].
	///
	/// Destructures self to avoid re-borrow gymnastics across phase functions.
	/// Tests call this directly to deterministically advance time without sleeps.
	#[cfg_attr(not(test), inline(always))]
	pub(crate) fn ensure_syntax_at(&mut self, now: Instant, ctx: EnsureSyntaxContext<'_>) -> SyntaxPollOutcome {
		let doc_id = ctx.doc_id;
		let SyntaxManager {
			cfg,
			policy,
			metrics,
			permits,
			entries,
			engine,
			collector,
		} = self;
		let entry = entries.entry(doc_id).or_insert_with(|| DocEntry::new(now));
		let ensure = derive(&ctx, policy);
		entry.last_tier = Some(ensure.tier);
		ensure_doc(now, ensure, entry, cfg, metrics, Arc::clone(permits), Arc::clone(engine), collector)
	}
}

/// Runs the full ensure pipeline for a single document.
#[allow(
	clippy::too_many_arguments,
	reason = "pipeline orchestration passes independent subsystems without hidden globals"
)]
fn ensure_doc(
	now: Instant,
	ctx: EnsureBase<'_>,
	entry: &mut DocEntry,
	cfg: &SyntaxManagerCfg,
	metrics: &mut SyntaxMetrics,
	permits: Arc<Semaphore>,
	engine: Arc<dyn SyntaxEngine>,
	collector: &mut TaskCollector,
) -> SyntaxPollOutcome {
	tracing::trace!(
		target: "xeno_undo_trace",
		doc_id = ?ctx.doc_id,
		doc_version = ctx.doc_version,
		bytes = ctx.bytes,
		tier = ?ctx.tier,
		hotness = ?ctx.hotness,
		language_id = ?ctx.language_id,
		viewport = ?ctx.viewport,
		work_disabled = ctx.work_disabled,
		"syntax.ensure.begin"
	);

	// Phase B: normalize
	let mut was_updated = normalize(entry, now, &ctx);

	// Phase C: install completions
	if install_completions(entry, now, &ctx, metrics) {
		was_updated = true;
	}

	// Phase D: gate
	let g = match gate(entry, now, ctx, was_updated) {
		ControlFlow::Break(outcome) => return outcome,
		ControlFlow::Continue(g) => g,
	};

	// Phase E: sync bootstrap
	if let ControlFlow::Break(outcome) = sync_bootstrap(entry, &g.ctx, engine.as_ref()) {
		return outcome;
	}

	// Phase F: compute plan
	let plan = compute_plan(entry, now, &g.ctx, &g.state, metrics);

	// Phase G: spawn plan
	let summary = spawn_plan(entry, &g.ctx, plan, collector, &permits, &engine, cfg);

	// Phase H: finalize
	let outcome = finalize(entry, now, &g.ctx, &g.state, was_updated, summary);
	tracing::trace!(
		target: "xeno_undo_trace",
		doc_id = ?g.ctx.base.doc_id,
		doc_version = g.ctx.base.doc_version,
		tier = ?g.ctx.base.tier,
		hotness = ?g.ctx.base.hotness,
		has_full = entry.slot.full.is_some(),
		has_any = entry.slot.has_any_tree(),
		dirty = entry.slot.dirty,
		viewport_uncovered = g.state.viewport_uncovered,
		planned_any = summary.planned_any,
		kicked_any = summary.kicked_any,
		active = entry.sched.any_active(),
		vp_cooling = entry.sched.lanes.viewport_urgent.in_cooldown(now),
		bg_cooling = entry.sched.lanes.bg.in_cooldown(now),
		result = ?outcome.result,
		updated = outcome.updated,
		"syntax.ensure.summary"
	);
	outcome
}

#[cfg(test)]
mod tests;
