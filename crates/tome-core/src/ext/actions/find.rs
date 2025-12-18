//! Find character actions (f/t/F/T commands).

use linkme::distributed_slice;

use crate::ext::actions::{ACTIONS, ActionDef, ActionResult, PendingAction, PendingKind};
use crate::movement;

#[distributed_slice(ACTIONS)]
static ACTION_FIND_CHAR: ActionDef = ActionDef {
	name: "find_char",
	description: "Select to next occurrence of character (inclusive)",
	handler: |ctx| match ctx.args.char {
		Some(ch) => {
			let mut new_sel = ctx.selection.clone();
			new_sel.transform_mut(|r| {
				*r = movement::find_char_forward(ctx.text, *r, ch, ctx.count, true, ctx.extend);
			});
			ActionResult::Motion(new_sel)
		}
		None => ActionResult::Pending(PendingAction {
			kind: PendingKind::FindChar { inclusive: true },
			prompt: "find→".into(),
		}),
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_FIND_CHAR_TO: ActionDef = ActionDef {
	name: "find_char_to",
	description: "Select to next occurrence of character (exclusive)",
	handler: |ctx| match ctx.args.char {
		Some(ch) => {
			let mut new_sel = ctx.selection.clone();
			new_sel.transform_mut(|r| {
				*r = movement::find_char_forward(ctx.text, *r, ch, ctx.count, false, ctx.extend);
			});
			ActionResult::Motion(new_sel)
		}
		None => ActionResult::Pending(PendingAction {
			kind: PendingKind::FindChar { inclusive: false },
			prompt: "to→".into(),
		}),
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_FIND_CHAR_REVERSE: ActionDef = ActionDef {
	name: "find_char_reverse",
	description: "Select to previous occurrence of character (inclusive)",
	handler: |ctx| match ctx.args.char {
		Some(ch) => {
			let mut new_sel = ctx.selection.clone();
			new_sel.transform_mut(|r| {
				*r = movement::find_char_backward(ctx.text, *r, ch, ctx.count, true, ctx.extend);
			});
			ActionResult::Motion(new_sel)
		}
		None => ActionResult::Pending(PendingAction {
			kind: PendingKind::FindCharReverse { inclusive: true },
			prompt: "find←".into(),
		}),
	},
};

#[distributed_slice(ACTIONS)]
static ACTION_FIND_CHAR_TO_REVERSE: ActionDef = ActionDef {
	name: "find_char_to_reverse",
	description: "Select to previous occurrence of character (exclusive)",
	handler: |ctx| match ctx.args.char {
		Some(ch) => {
			let mut new_sel = ctx.selection.clone();
			new_sel.transform_mut(|r| {
				*r = movement::find_char_backward(ctx.text, *r, ch, ctx.count, false, ctx.extend);
			});
			ActionResult::Motion(new_sel)
		}
		None => ActionResult::Pending(PendingAction {
			kind: PendingKind::FindCharReverse { inclusive: false },
			prompt: "to←".into(),
		}),
	},
};
