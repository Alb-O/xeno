use super::*;

/// Normalizes entry state for language/opts changes. Returns was_updated.
pub(super) fn normalize(entry: &mut DocEntry, now: Instant, ctx: &EnsureBase<'_>) -> bool {
	let mut updated = entry.slot.take_updated();

	if entry.slot.language_id != ctx.language_id {
		entry.sched.invalidate();
		if entry.slot.has_any_tree() {
			entry.slot.drop_tree();
			SyntaxManager::mark_updated(&mut entry.slot);
			updated = true;
		}
		entry.slot.language_id = ctx.language_id;
	}

	if matches!(ctx.hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
		entry.sched.last_visible_at = now;
	}

	if entry.slot.last_opts_key.is_some_and(|k| k != ctx.opts_key) {
		entry.sched.invalidate();
		entry.slot.dirty = true;
		entry.slot.drop_tree();
		SyntaxManager::mark_updated(&mut entry.slot);
		updated = true;
	}
	entry.slot.last_opts_key = Some(ctx.opts_key);

	updated
}
