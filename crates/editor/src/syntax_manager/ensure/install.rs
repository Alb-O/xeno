use super::*;

/// Decision for how to handle a successfully completed parse task.
#[derive(Debug)]
pub(super) enum InstallDecision {
	/// Install the tree into its slot.
	Install,
	/// Valid result but retention policy rejects it; drop existing trees.
	DropRetention,
	/// Discard result (stale, wrong language, wrong opts, work disabled, etc.).
	Discard,
}

/// Action computed for a completed parse result.
enum InstallAction {
	InstallViewport {
		done: CompletedRef,
		syntax: xeno_language::syntax::Syntax,
	},
	InstallFull {
		done: CompletedRef,
		syntax: xeno_language::syntax::Syntax,
	},
	DropRetention,
	ApplyFailureCooldown {
		done: CompletedRef,
		is_timeout: bool,
	},
	NoOp,
}

/// Result emitted by action application.
#[derive(Default)]
struct ApplyResult {
	was_updated: bool,
	is_installed: bool,
}

/// Task fields used by metrics and tracing after evaluation.
struct CompletionMeta {
	doc_version: u64,
	lang_id: xeno_language::LanguageId,
	class: TaskClass,
	injections: InjectionPolicy,
	elapsed: Duration,
	viewport_lane: Option<scheduling::ViewportLane>,
}

impl CompletionMeta {
	fn from_done(done: &CompletedSyntaxTask) -> Self {
		Self {
			doc_version: done.doc_version,
			lang_id: done.lang_id,
			class: done.class,
			injections: done.opts.injections,
			elapsed: done.elapsed,
			viewport_lane: done.viewport_lane,
		}
	}
}

/// Evaluation result for one completion item.
enum CompletionEval {
	Success {
		meta: CompletionMeta,
		decision: InstallDecision,
		action: InstallAction,
	},
	Timeout {
		meta: CompletionMeta,
		action: InstallAction,
	},
	Error {
		meta: CompletionMeta,
		action: InstallAction,
		error: xeno_language::syntax::SyntaxError,
	},
}

/// Checks whether a viewport task's injection policy matches the current config.
fn viewport_opts_ok(lane: Option<scheduling::ViewportLane>, injections: InjectionPolicy, cfg: &TierCfg) -> bool {
	match lane {
		Some(scheduling::ViewportLane::Urgent) => injections == cfg.viewport_injections || injections == InjectionPolicy::Eager,
		Some(scheduling::ViewportLane::Enrich) => injections == InjectionPolicy::Eager && cfg.viewport_stage_b_budget.is_some(),
		None => false,
	}
}

/// Checks whether a viewport completion should be installed (version, requested-min, continuity).
fn viewport_allow_install(done: &CompletedSyntaxTask, ctx: &EnsureSyntaxContext<'_>, d: &EnsureDerived, entry: &DocEntry) -> bool {
	let not_future = done.doc_version <= ctx.doc_version;
	let requested_min = match done.viewport_lane {
		Some(scheduling::ViewportLane::Enrich) => entry.sched.lanes.viewport_enrich.requested_doc_version,
		_ => entry.sched.lanes.viewport_urgent.requested_doc_version,
	};
	let requested_ok = done.doc_version >= requested_min;
	let continuity_needed = if done.doc_version < ctx.doc_version {
		match &d.viewport {
			Some(vp) => !(entry.slot.full.is_some() || entry.slot.viewport_cache.covers_range(vp)),
			None => true,
		}
	} else {
		true
	};
	not_future && requested_ok && continuity_needed
}

/// Checks whether a full-parse completion should be installed (version, projection continuity).
fn full_allow_install(done: &CompletedSyntaxTask, ctx: &EnsureSyntaxContext<'_>, entry: &DocEntry) -> bool {
	let preserves_projection = if done.doc_version < ctx.doc_version {
		entry.slot.full.is_none()
			|| entry
				.slot
				.pending_incremental
				.as_ref()
				.is_some_and(|p| p.base_tree_doc_version == done.doc_version)
	} else {
		true
	};
	SyntaxManager::should_install_completed_parse(
		done.doc_version,
		entry.slot.full.as_ref().map(|t| t.doc_version),
		entry.sched.lanes.bg.requested_doc_version,
		ctx.doc_version,
		entry.slot.dirty,
	) && preserves_projection
}

/// Pure decision: given a successful parse result, determine what to do with it.
pub(super) fn decide_install(done: &CompletedSyntaxTask, now: Instant, ctx: &EnsureSyntaxContext<'_>, d: &EnsureDerived, entry: &DocEntry) -> InstallDecision {
	let Some(current_lang) = ctx.language_id else {
		return InstallDecision::Discard;
	};
	if d.work_disabled {
		return InstallDecision::Discard;
	}

	let lang_ok = done.lang_id == current_lang;
	let is_viewport = done.class == TaskClass::Viewport;

	if is_viewport && (done.viewport_key.is_none() || done.viewport_lane.is_none()) {
		return InstallDecision::Discard;
	}

	let opts_ok = if is_viewport {
		viewport_opts_ok(done.viewport_lane, done.opts.injections, &d.cfg)
	} else {
		done.opts == d.opts_key
	};

	if !lang_ok || !opts_ok {
		return InstallDecision::Discard;
	}

	let retain_policy = if is_viewport {
		d.cfg.retention_hidden_viewport
	} else {
		d.cfg.retention_hidden_full
	};
	let retain_ok = SyntaxManager::retention_allows_install(now, &entry.sched, retain_policy, ctx.hotness);

	let allow = if is_viewport {
		viewport_allow_install(done, ctx, d, entry)
	} else {
		full_allow_install(done, ctx, entry)
	};

	if retain_ok && allow {
		InstallDecision::Install
	} else if !retain_ok && done.doc_version == ctx.doc_version {
		InstallDecision::DropRetention
	} else {
		InstallDecision::Discard
	}
}

/// Lightweight subset of completed task fields needed for apply helpers.
#[derive(Clone, Copy)]
struct CompletedRef {
	doc_version: u64,
	viewport_key: Option<ViewportKey>,
	viewport_lane: Option<scheduling::ViewportLane>,
}

impl CompletedRef {
	fn from_done(done: &CompletedSyntaxTask) -> Self {
		Self {
			doc_version: done.doc_version,
			viewport_key: done.viewport_key,
			viewport_lane: done.viewport_lane,
		}
	}
}

/// Applies a successful viewport parse install. Returns true (always updates).
fn apply_viewport_install(entry: &mut DocEntry, done: &CompletedRef, syntax: xeno_language::syntax::Syntax, current_lang: xeno_language::LanguageId) -> bool {
	let Some(vp_key) = done.viewport_key else { return false };
	let tree_id = entry.slot.alloc_tree_id();
	let coverage = if let Some(meta) = &syntax.viewport {
		meta.base_offset..meta.base_offset.saturating_add(meta.real_len)
	} else {
		0..0
	};
	let vp_tree = ViewportTree {
		syntax,
		doc_version: done.doc_version,
		tree_id,
		coverage,
	};
	let cache_entry = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
	if matches!(done.viewport_lane, Some(scheduling::ViewportLane::Urgent)) {
		cache_entry.stage_a = Some(vp_tree);
		cache_entry.stage_a_failed_for = None;
		cache_entry.attempted_b_for = None;
		entry.slot.dirty = true;
		entry.sched.force_no_debounce = true;
	} else {
		cache_entry.stage_b = Some(vp_tree);
		cache_entry.stage_b_cooldown_until = None;
	}
	entry.slot.language_id = Some(current_lang);
	SyntaxManager::mark_updated(&mut entry.slot);
	true
}

/// Applies a successful full-parse install. Returns true (always updates).
fn apply_full_install(
	entry: &mut DocEntry,
	done: &CompletedRef,
	syntax: xeno_language::syntax::Syntax,
	ctx: &EnsureSyntaxContext<'_>,
	current_lang: xeno_language::LanguageId,
) -> bool {
	let tree_id = entry.slot.alloc_tree_id();
	entry.slot.full = Some(InstalledTree {
		syntax,
		doc_version: done.doc_version,
		tree_id,
	});
	entry.slot.language_id = Some(current_lang);
	let keep_pending = done.doc_version < ctx.doc_version
		&& entry
			.slot
			.pending_incremental
			.as_ref()
			.is_some_and(|p| p.base_tree_doc_version == done.doc_version);
	if !keep_pending {
		entry.slot.pending_incremental = None;
	}
	entry.sched.force_no_debounce = false;
	if done.doc_version == ctx.doc_version {
		entry.slot.dirty = false;
		entry.sched.lanes.bg.cooldown_until = None;
	} else {
		entry.slot.dirty = true;
	}
	SyntaxManager::mark_updated(&mut entry.slot);
	true
}

/// Applies a retention-drop: clears all trees and marks clean.
fn apply_retention_drop(entry: &mut DocEntry) -> bool {
	entry.slot.drop_tree();
	entry.slot.dirty = false;
	entry.sched.force_no_debounce = false;
	SyntaxManager::mark_updated(&mut entry.slot);
	true
}

/// Applies cooldown/latch effects for a failed (timeout or error) task.
fn apply_failure_cooldowns(
	entry: &mut DocEntry,
	now: Instant,
	doc_version: u64,
	viewport_key: Option<ViewportKey>,
	viewport_lane: Option<scheduling::ViewportLane>,
	cfg: &TierCfg,
	is_timeout: bool,
) {
	let cooldown = if is_timeout {
		(cfg.viewport_cooldown_on_timeout, cfg.cooldown_on_timeout)
	} else {
		(cfg.viewport_cooldown_on_error, cfg.cooldown_on_error)
	};
	let is_enrich = viewport_lane == Some(scheduling::ViewportLane::Enrich);
	if is_enrich {
		if let Some(vp_key) = viewport_key {
			let ce = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
			ce.stage_b_cooldown_until = Some(now + cooldown.0);
			ce.attempted_b_for = None;
		}
	} else {
		if viewport_lane == Some(scheduling::ViewportLane::Urgent)
			&& let Some(vp_key) = viewport_key
		{
			let ce = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
			ce.stage_a_failed_for = Some(doc_version);
		}
		if viewport_lane == Some(scheduling::ViewportLane::Urgent) {
			entry.sched.lanes.viewport_urgent.set_cooldown(now + cooldown.0);
		} else {
			entry.sched.lanes.bg.set_cooldown(now + cooldown.1);
		}
	}
}

/// Computes the action for one completed item without mutating state.
fn evaluate_completion(done: CompletedSyntaxTask, now: Instant, ctx: &EnsureSyntaxContext<'_>, d: &EnsureDerived, entry: &DocEntry) -> CompletionEval {
	let meta = CompletionMeta::from_done(&done);
	let done_ref = CompletedRef::from_done(&done);
	let decision = decide_install(&done, now, ctx, d, entry);
	match done.result {
		Ok(syntax_tree) => {
			let action = match decision {
				InstallDecision::Install => {
					if done.class == TaskClass::Viewport {
						InstallAction::InstallViewport {
							done: done_ref,
							syntax: syntax_tree,
						}
					} else {
						InstallAction::InstallFull {
							done: done_ref,
							syntax: syntax_tree,
						}
					}
				}
				InstallDecision::DropRetention => InstallAction::DropRetention,
				InstallDecision::Discard => InstallAction::NoOp,
			};
			CompletionEval::Success { meta, decision, action }
		}
		Err(xeno_language::syntax::SyntaxError::Timeout) => CompletionEval::Timeout {
			meta,
			action: InstallAction::ApplyFailureCooldown {
				done: done_ref,
				is_timeout: true,
			},
		},
		Err(error) => CompletionEval::Error {
			meta,
			action: InstallAction::ApplyFailureCooldown {
				done: done_ref,
				is_timeout: false,
			},
			error,
		},
	}
}

/// Applies an install action and returns whether syntax state changed.
fn apply_install_action(entry: &mut DocEntry, now: Instant, ctx: &EnsureSyntaxContext<'_>, cfg: &TierCfg, action: InstallAction) -> ApplyResult {
	match action {
		InstallAction::InstallViewport { done, syntax } => {
			let Some(current_lang) = ctx.language_id else {
				return ApplyResult::default();
			};
			let is_installed = apply_viewport_install(entry, &done, syntax, current_lang);
			ApplyResult {
				was_updated: is_installed,
				is_installed,
			}
		}
		InstallAction::InstallFull { done, syntax } => {
			let Some(current_lang) = ctx.language_id else {
				return ApplyResult::default();
			};
			let was_updated = apply_full_install(entry, &done, syntax, ctx, current_lang);
			ApplyResult {
				was_updated,
				is_installed: true,
			}
		}
		InstallAction::DropRetention => {
			let was_updated = apply_retention_drop(entry);
			ApplyResult {
				was_updated,
				is_installed: false,
			}
		}
		InstallAction::ApplyFailureCooldown { done, is_timeout } => {
			apply_failure_cooldowns(entry, now, done.doc_version, done.viewport_key, done.viewport_lane, cfg, is_timeout);
			ApplyResult::default()
		}
		InstallAction::NoOp => ApplyResult::default(),
	}
}

/// Drains completed tasks, decides and applies install/discard/cooldown. Returns was_updated.
pub(super) fn install_completions(entry: &mut DocEntry, now: Instant, ctx: &EnsureSyntaxContext<'_>, d: &EnsureDerived, metrics: &mut SyntaxMetrics) -> bool {
	let mut was_updated = false;

	while let Some(done) = entry.sched.completed.pop_front() {
		match evaluate_completion(done, now, ctx, d, entry) {
			CompletionEval::Success { meta, decision, action } => {
				let result = apply_install_action(entry, now, ctx, &d.cfg, action);
				was_updated |= result.was_updated;
				metrics.record_task_result(
					meta.lang_id,
					d.tier,
					meta.class,
					meta.injections,
					meta.elapsed,
					false,
					false,
					result.is_installed,
				);
				tracing::trace!(
					target: "xeno_undo_trace",
					doc_id = ?ctx.doc_id,
					done_doc_version = meta.doc_version,
					ctx_doc_version = ctx.doc_version,
					class = ?meta.class,
					viewport_lane = ?meta.viewport_lane,
					injections = ?meta.injections,
					elapsed_ms = meta.elapsed.as_millis() as u64,
					is_installed = result.is_installed,
					?decision,
					"syntax.ensure.completed.ok"
				);
			}
			CompletionEval::Timeout { meta, action } => {
				let _ = apply_install_action(entry, now, ctx, &d.cfg, action);
				metrics.record_task_result(meta.lang_id, d.tier, meta.class, meta.injections, meta.elapsed, true, false, false);
				tracing::trace!(
					target: "xeno_undo_trace",
					doc_id = ?ctx.doc_id,
					done_doc_version = meta.doc_version,
					ctx_doc_version = ctx.doc_version,
					class = ?meta.class,
					viewport_lane = ?meta.viewport_lane,
					injections = ?meta.injections,
					elapsed_ms = meta.elapsed.as_millis() as u64,
					"syntax.ensure.completed.timeout"
				);
			}
			CompletionEval::Error { meta, action, error } => {
				tracing::warn!(doc_id = ?ctx.doc_id, tier = ?d.tier, error=%error, "Background syntax parse failed");
				let _ = apply_install_action(entry, now, ctx, &d.cfg, action);
				metrics.record_task_result(meta.lang_id, d.tier, meta.class, meta.injections, meta.elapsed, false, true, false);
				tracing::trace!(
					target: "xeno_undo_trace",
					doc_id = ?ctx.doc_id,
					done_doc_version = meta.doc_version,
					ctx_doc_version = ctx.doc_version,
					class = ?meta.class,
					viewport_lane = ?meta.viewport_lane,
					injections = ?meta.injections,
					elapsed_ms = meta.elapsed.as_millis() as u64,
					error = %error,
					"syntax.ensure.completed.error"
				);
			}
		}
	}

	was_updated
}
