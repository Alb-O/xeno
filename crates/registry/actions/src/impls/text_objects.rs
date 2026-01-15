use xeno_primitives::range::Range;
use xeno_registry_text_objects::{TextObjectDef, find_by_trigger};

use crate::{
	ActionContext, ActionEffects, ActionResult, ObjectSelectionKind, PendingAction, PendingKind,
	action,
};

/// Selects a text object by its trigger character.
fn select_object_with_trigger(
	ctx: &ActionContext,
	selection_kind: ObjectSelectionKind,
) -> ActionResult {
	let Some(trigger) = ctx.args.char else {
		return ActionResult::Effects(ActionEffects::pending(PendingAction {
			kind: PendingKind::Object(selection_kind),
			prompt: match selection_kind {
				ObjectSelectionKind::Inner => "inner".into(),
				ObjectSelectionKind::Around => "around".into(),
				ObjectSelectionKind::ToStart => "[obj".into(),
				ObjectSelectionKind::ToEnd => "]obj".into(),
			},
		}));
	};

	let Some(obj) = find_by_trigger(trigger) else {
		return ActionResult::Effects(ActionEffects::error(format!(
			"Unknown text object: {}",
			trigger
		)));
	};

	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		let pos = r.head;
		let result = match selection_kind {
			ObjectSelectionKind::Inner => (obj.inner)(ctx.text, pos),
			ObjectSelectionKind::Around => (obj.around)(ctx.text, pos),
			ObjectSelectionKind::ToStart => select_to_boundary(ctx, obj, pos, true),
			ObjectSelectionKind::ToEnd => select_to_boundary(ctx, obj, pos, false),
		};
		if let Some(new_range) = result {
			*r = new_range;
		}
	});

	ActionResult::Effects(ActionEffects::motion(new_sel))
}

/// Selects from cursor to the start or end boundary of a text object.
fn select_to_boundary(
	ctx: &ActionContext,
	obj: &TextObjectDef,
	pos: usize,
	to_start: bool,
) -> Option<Range> {
	let range = (obj.around)(ctx.text, pos)?;
	if to_start {
		Some(Range::new(pos, range.min()))
	} else {
		Some(Range::new(pos, range.max()))
	}
}

action!(select_object_inner, {
	description: "Select inner text object",
	bindings: r#"normal "alt-i""#,
}, |ctx| select_object_with_trigger(ctx, ObjectSelectionKind::Inner));

action!(select_object_around, {
	description: "Select around text object",
	bindings: r#"normal "alt-a""#,
}, |ctx| select_object_with_trigger(ctx, ObjectSelectionKind::Around));

action!(select_object_to_start, {
	description: "Select to object start",
	bindings: r#"normal "[" "{""#,
}, |ctx| select_object_with_trigger(ctx, ObjectSelectionKind::ToStart));

action!(select_object_to_end, {
	description: "Select to object end",
	bindings: r#"normal "]" "}""#,
}, |ctx| select_object_with_trigger(ctx, ObjectSelectionKind::ToEnd));
