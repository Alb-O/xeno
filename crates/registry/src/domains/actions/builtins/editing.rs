use crate::actions::{
	ActionEffects, ActionResult, PendingAction, PendingKind, action_handler, edit_op,
};

action_handler!(delete, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::delete(true))
));
action_handler!(delete_no_yank, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::delete(false))
));
action_handler!(change, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::change(true))
));
action_handler!(change_no_yank, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::change(false))
));
action_handler!(yank, |_ctx| ActionResult::Effects(ActionEffects::edit_op(
	edit_op::yank()
)));
action_handler!(paste_after, |_ctx| ActionResult::Effects(
	ActionEffects::paste(false)
));
action_handler!(paste_before, |_ctx| ActionResult::Effects(
	ActionEffects::paste(true)
));
action_handler!(undo, |_ctx| ActionResult::Effects(ActionEffects::edit_op(
	edit_op::undo()
)));
action_handler!(redo, |_ctx| ActionResult::Effects(ActionEffects::edit_op(
	edit_op::redo()
)));
action_handler!(indent, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::indent())
));
action_handler!(deindent, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::deindent())
));
action_handler!(join_lines, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::join_lines())
));
action_handler!(delete_back, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::delete_back())
));
action_handler!(delete_forward, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::delete_forward())
));
action_handler!(delete_word_back, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::delete_word_back())
));
action_handler!(delete_word_forward, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::delete_word_forward())
));
action_handler!(paste_all_after, |_ctx| ActionResult::Effects(
	ActionEffects::paste(false)
));
action_handler!(paste_all_before, |_ctx| ActionResult::Effects(
	ActionEffects::paste(true)
));
action_handler!(to_lowercase, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::case_convert(edit_op::CharMapKind::ToLowerCase))
));
action_handler!(to_uppercase, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::case_convert(edit_op::CharMapKind::ToUpperCase))
));
action_handler!(swap_case, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::case_convert(edit_op::CharMapKind::SwapCase))
));
action_handler!(open_below, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::open_below())
));
action_handler!(open_above, |_ctx| ActionResult::Effects(
	ActionEffects::edit_op(edit_op::open_above())
));

action_handler!(replace_char, |ctx| match ctx.args.char {
	Some(ch) => ActionResult::Effects(ActionEffects::edit_op(edit_op::replace_with_char(ch))),
	None => ActionResult::Effects(ActionEffects::pending(PendingAction {
		kind: PendingKind::ReplaceChar,
		prompt: "replace".into(),
	})),
});
