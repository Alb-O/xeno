use crate::edit_op::{self, CharMapKind};
use crate::{ActionEffects, ActionResult, PendingAction, PendingKind, action};

action!(delete, { description: "Delete selection", bindings: r#"normal "d""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::delete(true))));

action!(delete_no_yank, { description: "Delete selection (no yank)", bindings: r#"normal "alt-d""# },
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

action!(paste_all_after, { description: "Paste all after", bindings: r#"normal "alt-p""# },
	|_ctx| ActionResult::Effects(ActionEffects::paste(false)));

action!(paste_all_before, { description: "Paste all before", bindings: r#"normal "alt-P""# },
	|_ctx| ActionResult::Effects(ActionEffects::paste(true)));

action!(undo, { description: "Undo last change", bindings: r#"normal "u""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::undo())));

action!(redo, { description: "Redo last change", bindings: r#"normal "U""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::redo())));

action!(indent, { description: "Indent selection", bindings: r#"normal ">""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::indent())));

action!(deindent, { description: "Deindent selection", bindings: r#"normal "<""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::deindent())));

action!(to_lowercase, { description: "Convert to lowercase", bindings: r#"normal "`""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::case_convert(CharMapKind::ToLowerCase))));

action!(to_uppercase, { description: "Convert to uppercase", bindings: r#"normal "~""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::case_convert(CharMapKind::ToUpperCase))));

action!(swap_case, { description: "Swap case", bindings: r#"normal "alt-`""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::case_convert(CharMapKind::SwapCase))));

action!(join_lines, { description: "Join lines", bindings: r#"normal "alt-j""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::join_lines())));

action!(open_below, { description: "Open line below", bindings: r#"normal "o""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::open_below())));

action!(open_above, { description: "Open line above", bindings: r#"normal "O""# },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::open_above())));

action!(delete_back, { description: "Delete character before cursor" },
	|_ctx| ActionResult::Effects(ActionEffects::edit_op(edit_op::delete_back())));

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
