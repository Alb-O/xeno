//! Editor context and effect handling.

use std::time::Instant;

use tracing::{trace, trace_span};
use xeno_primitives::range::Range;
use xeno_primitives::{Mode, Selection};
pub use xeno_registry::actions::editor_ctx::*;
use xeno_registry::{
	ActionEffects, ActionResult, AppEffect, EditEffect, Effect, HookContext, HookEventData,
	ScreenPosition, ScrollAmount, UiEffect, ViewEffect, emit_sync as emit_hook_sync,
	notification_keys as keys, result_handler,
};

/// Applies a set of effects to the editor context.
///
/// Effects are applied in order. Hook emissions are centralized here,
/// avoiding the duplication present in individual result handlers.
///
/// Returns `true` if the editor should quit.
pub fn apply_effects(
	effects: &ActionEffects,
	ctx: &mut xeno_registry::actions::editor_ctx::EditorContext,
	extend: bool,
) -> HandleOutcome {
	if !effects.is_empty() {
		trace!(count = effects.len(), "applying effects");
	}

	let mut outcome = HandleOutcome::Handled;

	for effect in effects {
		let (kind, triggers_lsp_sync) = effect_kind(effect);
		let span = trace_span!("editor.effect", kind, triggers_lsp_sync);
		let _guard = span.enter();
		let start = Instant::now();

		match effect {
			Effect::View(view) => apply_view_effect(view, ctx, extend),
			Effect::Edit(edit) => apply_edit_effect(edit, ctx),
			Effect::Ui(ui) => apply_ui_effect(ui, ctx),
			Effect::App(app) => {
				if let Some(quit) = apply_app_effect(app, ctx) {
					outcome = quit;
				}
			}
			_ => {
				debug_assert!(false, "Unhandled effect variant: {effect:?}");
				trace!(?effect, "unhandled effect variant");
			}
		}
		trace!(
			duration_ms = start.elapsed().as_millis() as u64,
			"effect.applied"
		);
	}

	outcome
}

fn effect_kind(effect: &Effect) -> (&'static str, bool) {
	match effect {
		Effect::View(_) => ("view", false),
		Effect::Edit(_) => ("edit", true),
		Effect::Ui(_) => ("ui", false),
		Effect::App(_) => ("app", false),
		_ => ("unknown", false),
	}
}

/// Applies a view-related effect.
fn apply_view_effect(
	effect: &ViewEffect,
	ctx: &mut xeno_registry::actions::editor_ctx::EditorContext,
	extend: bool,
) {
	match effect {
		ViewEffect::SetCursor(pos) => {
			ctx.set_cursor(*pos);
			emit_cursor_hook(ctx);
		}

		ViewEffect::SetSelection(sel) => {
			ctx.set_cursor(sel.primary().head);
			ctx.set_selection(sel.clone());
			emit_cursor_hook(ctx);
			emit_selection_hook(ctx, sel);
		}

		ViewEffect::Motion(req) => {
			if let Some(dispatch) = ctx.motion_dispatch() {
				let sel = dispatch.apply_motion(req);
				ctx.set_cursor(sel.primary().head);
				ctx.set_selection(sel.clone());
				emit_cursor_hook(ctx);
				emit_selection_hook(ctx, &sel);
			} else {
				trace!("motion dispatch not available");
			}
		}

		ViewEffect::ScreenMotion { position, count } => {
			apply_screen_motion(ctx, *position, *count, extend);
		}

		ViewEffect::Scroll {
			direction,
			amount,
			extend: scroll_extend,
		} => {
			let count = scroll_amount_to_lines(amount);
			if let Some(motion) = ctx.motion() {
				motion.move_visual_vertical(*direction, count, *scroll_extend);
			}
		}

		ViewEffect::VisualMove {
			direction,
			count,
			extend: move_extend,
		} => {
			if let Some(motion) = ctx.motion() {
				motion.move_visual_vertical(*direction, *count, *move_extend);
			}
		}

		ViewEffect::Search {
			direction,
			add_selection,
		} => {
			if let Some(search) = ctx.search() {
				search.search(*direction, *add_selection, extend);
			}
		}

		ViewEffect::UseSelectionAsSearch => {
			if let Some(search) = ctx.search() {
				search.use_selection_as_pattern();
			}
		}

		_ => {
			debug_assert!(false, "Unhandled view effect variant: {effect:?}");
			trace!(?effect, "unhandled view effect variant");
		}
	}
}

/// Applies a text editing effect.
fn apply_edit_effect(
	effect: &EditEffect,
	ctx: &mut xeno_registry::actions::editor_ctx::EditorContext,
) {
	match effect {
		EditEffect::EditOp(op) => {
			if let Some(edit) = ctx.edit() {
				edit.execute_edit_op(op);
			}
		}

		EditEffect::Paste { before } => {
			if let Some(edit) = ctx.edit() {
				edit.paste(*before);
			}
		}

		_ => {
			debug_assert!(false, "Unhandled edit effect variant: {effect:?}");
			trace!(?effect, "unhandled edit effect variant");
		}
	}
}

/// Applies a UI-related effect.
fn apply_ui_effect(effect: &UiEffect, ctx: &mut xeno_registry::actions::editor_ctx::EditorContext) {
	match effect {
		UiEffect::Notify(notification) => {
			ctx.emit(notification.clone());
		}

		UiEffect::Error(msg) => {
			ctx.emit(keys::action_error(msg));
		}

		UiEffect::OpenPalette => {
			ctx.open_palette();
		}

		UiEffect::ClosePalette => {
			ctx.close_palette();
		}

		UiEffect::ExecutePalette => {
			ctx.execute_palette();
		}

		UiEffect::ForceRedraw => {}

		_ => {
			debug_assert!(false, "Unhandled ui effect variant: {effect:?}");
			trace!(?effect, "unhandled ui effect variant");
		}
	}
}

/// Applies an application-level effect.
///
/// Returns `Some(HandleOutcome::Quit)` if this is a quit effect.
fn apply_app_effect(
	effect: &AppEffect,
	ctx: &mut xeno_registry::actions::editor_ctx::EditorContext,
) -> Option<HandleOutcome> {
	match effect {
		AppEffect::SetMode(mode) => {
			ctx.set_mode(mode.clone());
		}

		AppEffect::Pending(pending) => {
			ctx.emit(keys::pending_prompt(&pending.prompt));
			ctx.set_mode(Mode::PendingAction(pending.kind));
		}

		AppEffect::FocusBuffer(direction) => {
			if let Some(ops) = ctx.focus_ops() {
				ops.buffer_switch(*direction);
			}
		}

		AppEffect::FocusSplit(direction) => {
			if let Some(ops) = ctx.focus_ops() {
				ops.focus(*direction);
			}
		}

		AppEffect::Split(axis) => {
			if let Some(ops) = ctx.split_ops() {
				ops.split(*axis);
			}
		}

		AppEffect::CloseSplit => {
			if let Some(ops) = ctx.split_ops() {
				ops.close_split();
			}
		}

		AppEffect::CloseOtherBuffers => {
			if let Some(ops) = ctx.split_ops() {
				ops.close_other_buffers();
			}
		}

		AppEffect::Quit { force: _ } => {
			return Some(HandleOutcome::Quit);
		}

		AppEffect::QueueCommand { name, args } => {
			if let Some(queue) = ctx.command_queue() {
				queue.queue_command(name, args.clone());
			}
		}

		_ => {
			debug_assert!(false, "Unhandled app effect variant: {effect:?}");
			trace!(?effect, "unhandled app effect variant");
		}
	}

	None
}

/// Converts scroll amount to line count.
fn scroll_amount_to_lines(amount: &ScrollAmount) -> usize {
	match amount {
		ScrollAmount::Line(n) => *n,
		ScrollAmount::HalfPage => 10,
		ScrollAmount::FullPage => 20,
	}
}

/// Emits cursor move hook if position is available.
fn emit_cursor_hook(ctx: &xeno_registry::actions::editor_ctx::EditorContext) {
	if let Some((line, col)) = ctx.cursor_line_col() {
		emit_hook_sync(&HookContext::new(HookEventData::CursorMove { line, col }));
	}
}

/// Emits selection change hook.
fn emit_selection_hook(_ctx: &xeno_registry::actions::editor_ctx::EditorContext, sel: &Selection) {
	let primary = sel.primary();
	emit_hook_sync(&HookContext::new(HookEventData::SelectionChange {
		anchor: primary.anchor,
		head: primary.head,
	}));
}

/// Applies a screen-relative motion (H/M/L).
fn apply_screen_motion(
	ctx: &mut xeno_registry::actions::editor_ctx::EditorContext,
	position: ScreenPosition,
	count: usize,
	extend: bool,
) {
	let Some(viewport) = ctx.viewport() else {
		ctx.emit(keys::VIEWPORT_UNAVAILABLE);
		return;
	};

	let height = viewport.viewport_height();
	if height == 0 {
		ctx.emit(keys::VIEWPORT_HEIGHT_UNAVAILABLE);
		return;
	}

	let count = count.max(1);
	let mut row = match position {
		ScreenPosition::Top => count.saturating_sub(1),
		ScreenPosition::Middle => height / 2 + count.saturating_sub(1),
		ScreenPosition::Bottom => height.saturating_sub(count),
	};
	if row >= height {
		row = height.saturating_sub(1);
	}

	let Some(target) = viewport.viewport_row_to_doc_position(row) else {
		ctx.emit(keys::SCREEN_MOTION_UNAVAILABLE);
		return;
	};

	let selection = ctx.selection();
	let primary_index = selection.primary_index();
	let new_ranges: Vec<Range> = selection
		.ranges()
		.iter()
		.map(|range| {
			if extend {
				Range::new(range.anchor, target)
			} else {
				Range::point(target)
			}
		})
		.collect();
	let new_selection = Selection::from_vec(new_ranges, primary_index);

	ctx.set_cursor(new_selection.primary().head);
	ctx.set_selection(new_selection.clone());
	emit_cursor_hook(ctx);
	emit_selection_hook(ctx, &new_selection);
}

// Register the handler for ActionResult::Effects
result_handler!(
	RESULT_EFFECTS_HANDLERS,
	HANDLE_EFFECTS,
	"effects",
	|r, ctx, extend| {
		let ActionResult::Effects(effects) = r;
		apply_effects(effects, ctx, extend)
	}
);

pub(crate) fn register_result_handlers() {
	xeno_registry::actions::register_result_handler(&HANDLE_EFFECTS);
}

#[cfg(test)]
mod tests;
