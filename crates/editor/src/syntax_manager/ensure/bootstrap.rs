use std::ops::ControlFlow;

use super::*;

/// Attempts a synchronous bootstrap parse for first-visible documents.
pub(super) fn sync_bootstrap(entry: &mut DocEntry, ctx: &EnsureSyntaxContext<'_>, d: &EnsureDerived, engine: &dyn SyntaxEngine) -> Flow<()> {
	let cfg = d.cfg;
	let is_bootstrap = !entry.slot.has_any_tree();
	let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);

	if !(is_bootstrap && is_visible && !entry.slot.sync_bootstrap_attempted) {
		return ControlFlow::Continue(());
	}
	let Some(sync_timeout) = cfg.sync_bootstrap_timeout else {
		return ControlFlow::Continue(());
	};

	entry.slot.sync_bootstrap_attempted = true;
	let pre_epoch = entry.sched.epoch;
	let lang_id = ctx.language_id.unwrap();

	let sync_opts = SyntaxOptions {
		parse_timeout: sync_timeout,
		injections: cfg.injections,
	};
	tracing::trace!(
		target: "xeno_undo_trace",
		doc_id = ?ctx.doc_id,
		doc_version = ctx.doc_version,
		sync_timeout_ms = sync_opts.parse_timeout.as_millis() as u64,
		injections = ?sync_opts.injections,
		"syntax.ensure.sync_bootstrap.attempt"
	);

	match engine.parse(ctx.content.slice(..), lang_id, ctx.loader, sync_opts) {
		Ok(syntax) => {
			let is_bootstrap = !entry.slot.has_any_tree();
			let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);
			if entry.sched.epoch == pre_epoch && is_bootstrap && is_visible && !entry.sched.any_active() {
				let tree_id = entry.slot.alloc_tree_id();
				entry.slot.full = Some(InstalledTree {
					syntax,
					doc_version: ctx.doc_version,
					tree_id,
				});
				entry.slot.language_id = Some(lang_id);
				entry.slot.dirty = false;
				entry.slot.pending_incremental = None;
				entry.sched.force_no_debounce = false;
				entry.sched.lanes.bg.cooldown_until = None;
				SyntaxManager::mark_updated(&mut entry.slot);
				tracing::trace!(
					target: "xeno_undo_trace",
					doc_id = ?ctx.doc_id,
					doc_version = ctx.doc_version,
					tree_id,
					"syntax.ensure.sync_bootstrap.installed"
				);
				return ControlFlow::Break(SyntaxPollOutcome {
					result: SyntaxPollResult::Ready,
					updated: true,
				});
			}
		}
		Err(e) => {
			tracing::trace!(
				target: "xeno_undo_trace",
				doc_id = ?ctx.doc_id,
				doc_version = ctx.doc_version,
				error = %e,
				"syntax.ensure.sync_bootstrap.failed"
			);
		}
	}

	ControlFlow::Continue(())
}
