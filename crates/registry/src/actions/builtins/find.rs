use xeno_primitives::movement;

use crate::actions::{ActionEffects, ActionResult, PendingAction, PendingKind, action_handler};

action_handler!(find_char, |ctx| match ctx.args.char {
	Some(ch) => {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			*r = movement::find_char_forward(ctx.text, *r, ch, ctx.count, true, ctx.extend);
		});
		ActionResult::Effects(ActionEffects::selection(new_sel))
	}
	None => ActionResult::Effects(ActionEffects::pending(PendingAction {
		kind: PendingKind::FindChar { inclusive: true },
		prompt: "find->".into(),
	})),
});

action_handler!(find_char_to, |ctx| match ctx.args.char {
	Some(ch) => {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			*r = movement::find_char_forward(ctx.text, *r, ch, ctx.count, false, ctx.extend);
		});
		ActionResult::Effects(ActionEffects::selection(new_sel))
	}
	None => ActionResult::Effects(ActionEffects::pending(PendingAction {
		kind: PendingKind::FindChar { inclusive: false },
		prompt: "to->".into(),
	})),
});

action_handler!(find_char_reverse, |ctx| match ctx.args.char {
	Some(ch) => {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			*r = movement::find_char_backward(ctx.text, *r, ch, ctx.count, true, ctx.extend);
		});
		ActionResult::Effects(ActionEffects::selection(new_sel))
	}
	None => ActionResult::Effects(ActionEffects::pending(PendingAction {
		kind: PendingKind::FindCharReverse { inclusive: true },
		prompt: "find<-".into(),
	})),
});

action_handler!(find_char_to_reverse, |ctx| match ctx.args.char {
	Some(ch) => {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			*r = movement::find_char_backward(ctx.text, *r, ch, ctx.count, false, ctx.extend);
		});
		ActionResult::Effects(ActionEffects::selection(new_sel))
	}
	None => ActionResult::Effects(ActionEffects::pending(PendingAction {
		kind: PendingKind::FindCharReverse { inclusive: false },
		prompt: "to<-".into(),
	})),
});
