use crate::{ActionResult, EditAction, PendingAction, PendingKind, action};

action!(delete, { description: "Delete selection", bindings: r#"normal "d""# },
	|_ctx| ActionResult::Edit(EditAction::Delete { yank: true }));

action!(delete_no_yank, { description: "Delete selection (no yank)", bindings: r#"normal "alt-d""# },
	|_ctx| ActionResult::Edit(EditAction::Delete { yank: false }));

action!(change, { description: "Change selection", bindings: r#"normal "c""# },
	|_ctx| ActionResult::Edit(EditAction::Change { yank: true }));

action!(change_no_yank, { description: "Change selection (no yank)", bindings: r#"normal "alt-c""# },
	|_ctx| ActionResult::Edit(EditAction::Change { yank: false }));

action!(yank, { description: "Yank selection", bindings: r#"normal "y""# },
	|_ctx| ActionResult::Edit(EditAction::Yank));

action!(paste_after, { description: "Paste after cursor", bindings: r#"normal "p""# },
	|_ctx| ActionResult::Edit(EditAction::Paste { before: false }));

action!(paste_before, { description: "Paste before cursor", bindings: r#"normal "P""# },
	|_ctx| ActionResult::Edit(EditAction::Paste { before: true }));

action!(paste_all_after, { description: "Paste all after", bindings: r#"normal "alt-p""# },
	|_ctx| ActionResult::Edit(EditAction::PasteAll { before: false }));

action!(paste_all_before, { description: "Paste all before", bindings: r#"normal "alt-P""# },
	|_ctx| ActionResult::Edit(EditAction::PasteAll { before: true }));

action!(undo, { description: "Undo last change", bindings: r#"normal "u""# },
	|_ctx| ActionResult::Edit(EditAction::Undo));

action!(redo, { description: "Redo last change", bindings: r#"normal "U""# },
	|_ctx| ActionResult::Edit(EditAction::Redo));

action!(indent, { description: "Indent selection", bindings: r#"normal ">""# },
	|_ctx| ActionResult::Edit(EditAction::Indent));

action!(deindent, { description: "Deindent selection", bindings: r#"normal "<""# },
	|_ctx| ActionResult::Edit(EditAction::Deindent));

action!(to_lowercase, { description: "Convert to lowercase", bindings: r#"normal "`""# },
	|_ctx| ActionResult::Edit(EditAction::ToLowerCase));

action!(to_uppercase, { description: "Convert to uppercase", bindings: r#"normal "~""# },
	|_ctx| ActionResult::Edit(EditAction::ToUpperCase));

action!(swap_case, { description: "Swap case", bindings: r#"normal "alt-`""# },
	|_ctx| ActionResult::Edit(EditAction::SwapCase));

action!(join_lines, { description: "Join lines", bindings: r#"normal "alt-j""# },
	|_ctx| ActionResult::Edit(EditAction::JoinLines));

action!(open_below, { description: "Open line below", bindings: r#"normal "o""# },
	|_ctx| ActionResult::Edit(EditAction::OpenBelow));

action!(open_above, { description: "Open line above", bindings: r#"normal "O""# },
	|_ctx| ActionResult::Edit(EditAction::OpenAbove));

action!(delete_back, { description: "Delete character before cursor" },
	|_ctx| ActionResult::Edit(EditAction::DeleteBack));

action!(replace_char, {
	description: "Replace selection with character",
	bindings: r#"normal "r""#,
}, |ctx| match ctx.args.char {
	Some(ch) => ActionResult::Edit(EditAction::ReplaceWithChar { ch }),
	None => ActionResult::Pending(PendingAction {
		kind: PendingKind::ReplaceChar,
		prompt: "replace".into(),
	}),
});
