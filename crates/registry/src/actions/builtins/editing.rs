use crate::actions::{ActionEffects, ActionResult, PendingAction, PendingKind, action, edit_op};

action!(delete, { description: "Delete selection", bindings: r#"normal "d""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::delete(true))));

action!(delete_no_yank, { description: "Delete selection (no yank)", bindings: r#"normal "alt-d" "delete""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::delete(false))));

action!(change, { description: "Change selection", bindings: r#"normal "c""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::change(true))));

action!(change_no_yank, { description: "Change selection (no yank)", bindings: r#"normal "alt-c""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::change(false))));

action!(yank, { description: "Yank selection", bindings: r#"normal "y""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::yank())));

action!(paste_after, { description: "Paste after cursor", bindings: r#"normal "p""# },
	|_ctx| ActionResult::Effects(ActionEffects::paste(false)));

action!(paste_before, { description: "Paste before cursor", bindings: r#"normal "P""# },
	|_ctx| ActionResult::Effects(ActionEffects::paste(true)));

action!(undo, { description: "Undo last change", bindings: r#"normal "u""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::undo())));

action!(redo, { description: "Redo last change", bindings: r#"normal "U""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::redo())));

action!(indent, { description: "Indent line", bindings: r#"normal ">""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::indent())));

action!(deindent, { description: "Deindent line", bindings: r#"normal "<""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::deindent())));

action!(join_lines, { description: "Join lines", bindings: r#"normal "J""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::join_lines())));

action!(delete_back, { description: "Delete character before cursor", bindings: r#"normal "backspace""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::delete_back())));

action!(delete_forward, { description: "Delete character after cursor", bindings: r#""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::delete_forward())));

action!(delete_word_back, {
	description: "Delete word before cursor",
	bindings: r#"normal "ctrl-backspace"
insert "ctrl-backspace""#,
}, |_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::delete_word_back())));

action!(delete_word_forward, {
	description: "Delete word after cursor",
	bindings: r#"normal "ctrl-delete"
insert "ctrl-delete""#,
}, |_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::delete_word_forward())));

action!(paste_all_after, { description: "Paste all after", bindings: r#"normal "alt-p""# },
	|_ctx| ActionResult::Effects(ActionEffects::paste(false)));

action!(paste_all_before, { description: "Paste all before", bindings: r#"normal "alt-P""# },
	|_ctx| ActionResult::Effects(ActionEffects::paste(true)));

action!(to_lowercase, { description: "Convert to lowercase", bindings: r#"normal "`""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::case_convert(edit_op::CharMapKind::ToLowerCase))));

action!(to_uppercase, { description: "Convert to uppercase", bindings: r#"normal "~""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::case_convert(edit_op::CharMapKind::ToUpperCase))));

action!(swap_case, { description: "Swap case", bindings: r#"normal "alt-`""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::case_convert(edit_op::CharMapKind::SwapCase))));

action!(open_below, { description: "Open line below", bindings: r#"normal "o""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::open_below())));

action!(open_above, { description: "Open line above", bindings: r#"normal "O""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::open_above())));

action!(replace_char, {
	description: "Replace selection with character",
	bindings: r#"normal "r""#,
}, |ctx| match ctx.args.char {
	Some(ch) => ActionResult::Effects(ActionEffects::edit_op(edit_op::replace_with_char(ch))),
	None => ActionResult::Effects(ActionEffects::pending(PendingAction {
		kind: PendingKind::ReplaceChar,
		prompt: "replace".into(),
	})),
});

pub(super) const DEFS: &[&crate::actions::ActionDef] = &[
	&ACTION_delete,
	&ACTION_delete_no_yank,
	&ACTION_change,
	&ACTION_change_no_yank,
	&ACTION_yank,
	&ACTION_paste_after,
	&ACTION_paste_before,
	&ACTION_undo,
	&ACTION_redo,
	&ACTION_indent,
	&ACTION_deindent,
	&ACTION_join_lines,
	&ACTION_delete_back,
	&ACTION_delete_forward,
	&ACTION_delete_word_back,
	&ACTION_delete_word_forward,
	&ACTION_paste_all_after,
	&ACTION_paste_all_before,
	&ACTION_to_lowercase,
	&ACTION_to_uppercase,
	&ACTION_swap_case,
	&ACTION_open_below,
	&ACTION_open_above,
	&ACTION_replace_char,
];
