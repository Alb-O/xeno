//! Text object selection actions.

use linkme::distributed_slice;

use crate::ext::actions::{
	ACTIONS, ActionDef, ActionResult, ObjectSelectionKind, PendingAction, PendingKind,
};
use crate::ext::{TextObjectDef, find_text_object};
use crate::range::Range;

fn select_object_with_trigger(
	ctx: &crate::ext::actions::ActionContext,
	selection_kind: ObjectSelectionKind,
) -> ActionResult {
	let Some(trigger) = ctx.args.char else {
		return ActionResult::Pending(PendingAction {
			kind: PendingKind::Object(selection_kind),
			prompt: match selection_kind {
				ObjectSelectionKind::Inner => "inner".into(),
				ObjectSelectionKind::Around => "around".into(),
				ObjectSelectionKind::ToStart => "[obj".into(),
				ObjectSelectionKind::ToEnd => "]obj".into(),
			},
		});
	};

	let Some(obj) = find_text_object(trigger) else {
		return ActionResult::Error(format!("Unknown text object: {}", trigger));
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

	ActionResult::Motion(new_sel)
}

fn select_to_boundary(
	ctx: &crate::ext::actions::ActionContext,
	obj: &TextObjectDef,
	pos: usize,
	to_start: bool,
) -> Option<Range> {
	let range = (obj.around)(ctx.text, pos)?;
	if to_start {
		Some(Range::new(pos, range.from()))
	} else {
		Some(Range::new(pos, range.to()))
	}
}

#[distributed_slice(ACTIONS)]
static ACTION_SELECT_OBJECT_INNER: ActionDef = ActionDef {
	name: "select_object_inner",
	description: "Select inner text object",
	handler: |ctx| select_object_with_trigger(ctx, ObjectSelectionKind::Inner),
};

#[distributed_slice(ACTIONS)]
static ACTION_SELECT_OBJECT_AROUND: ActionDef = ActionDef {
	name: "select_object_around",
	description: "Select around text object",
	handler: |ctx| select_object_with_trigger(ctx, ObjectSelectionKind::Around),
};

#[distributed_slice(ACTIONS)]
static ACTION_SELECT_OBJECT_TO_START: ActionDef = ActionDef {
	name: "select_object_to_start",
	description: "Select to object start",
	handler: |ctx| select_object_with_trigger(ctx, ObjectSelectionKind::ToStart),
};

#[distributed_slice(ACTIONS)]
static ACTION_SELECT_OBJECT_TO_END: ActionDef = ActionDef {
	name: "select_object_to_end",
	description: "Select to object end",
	handler: |ctx| select_object_with_trigger(ctx, ObjectSelectionKind::ToEnd),
};
