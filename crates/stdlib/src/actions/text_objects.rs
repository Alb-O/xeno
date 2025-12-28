//! Text object selection actions.

use evildoer_base::key::Key;
use evildoer_base::range::Range;
use evildoer_manifest::actions::{ActionResult, ObjectSelectionKind, PendingAction, PendingKind};
use evildoer_manifest::{TextObjectDef, bound_action, find_text_object_by_trigger};

fn select_object_with_trigger(
	ctx: &evildoer_manifest::actions::ActionContext,
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

	let Some(obj) = find_text_object_by_trigger(trigger) else {
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
	ctx: &evildoer_manifest::actions::ActionContext,
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

bound_action!(
	select_object_inner,
	description: "Select inner text object",
	bindings: [Normal => [Key::alt('i')]],
	|ctx| select_object_with_trigger(ctx, ObjectSelectionKind::Inner)
);

bound_action!(
	select_object_around,
	description: "Select around text object",
	bindings: [Normal => [Key::alt('a')]],
	|ctx| select_object_with_trigger(ctx, ObjectSelectionKind::Around)
);

bound_action!(
	select_object_to_start,
	description: "Select to object start",
	bindings: [Normal => [Key::char('['), Key::char('{')]],
	|ctx| select_object_with_trigger(ctx, ObjectSelectionKind::ToStart)
);

bound_action!(
	select_object_to_end,
	description: "Select to object end",
	bindings: [Normal => [Key::char(']'), Key::char('}')]],
	|ctx| select_object_with_trigger(ctx, ObjectSelectionKind::ToEnd)
);
