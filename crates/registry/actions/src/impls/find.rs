use evildoer_registry_motions::movement;

use crate::{action, ActionResult, PendingAction, PendingKind};

action!(find_char, {
	description: "Select to next occurrence of character (inclusive)",
	bindings: r#"normal "f""#,
}, |ctx| match ctx.args.char {
	Some(ch) => {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			*r = movement::find_char_forward(ctx.text, *r, ch, ctx.count, true, ctx.extend);
		});
		ActionResult::Motion(new_sel)
	}
	None => ActionResult::Pending(PendingAction {
		kind: PendingKind::FindChar { inclusive: true },
		prompt: "find->".into(),
	}),
});

action!(find_char_to, {
	description: "Select to next occurrence of character (exclusive)",
	bindings: r#"normal "t""#,
}, |ctx| match ctx.args.char {
	Some(ch) => {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			*r = movement::find_char_forward(ctx.text, *r, ch, ctx.count, false, ctx.extend);
		});
		ActionResult::Motion(new_sel)
	}
	None => ActionResult::Pending(PendingAction {
		kind: PendingKind::FindChar { inclusive: false },
		prompt: "to->".into(),
	}),
});

action!(find_char_reverse, {
	description: "Select to previous occurrence of character (inclusive)",
	bindings: r#"normal "alt-f""#,
}, |ctx| match ctx.args.char {
	Some(ch) => {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			*r = movement::find_char_backward(ctx.text, *r, ch, ctx.count, true, ctx.extend);
		});
		ActionResult::Motion(new_sel)
	}
	None => ActionResult::Pending(PendingAction {
		kind: PendingKind::FindCharReverse { inclusive: true },
		prompt: "find<-".into(),
	}),
});

action!(find_char_to_reverse, {
	description: "Select to previous occurrence of character (exclusive)",
	bindings: r#"normal "alt-t""#,
}, |ctx| match ctx.args.char {
	Some(ch) => {
		let mut new_sel = ctx.selection.clone();
		new_sel.transform_mut(|r| {
			*r = movement::find_char_backward(ctx.text, *r, ch, ctx.count, false, ctx.extend);
		});
		ActionResult::Motion(new_sel)
	}
	None => ActionResult::Pending(PendingAction {
		kind: PendingKind::FindCharReverse { inclusive: false },
		prompt: "to<-".into(),
	}),
});
