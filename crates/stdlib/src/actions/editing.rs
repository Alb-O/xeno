//! Editing actions (delete, yank, paste, undo, redo, etc.).

use evildoer_manifest::actions::{ActionDef, ActionResult, EditAction, PendingAction, PendingKind};
use evildoer_manifest::{ACTIONS, bound_action};
use linkme::distributed_slice;

bound_action!(delete, description: "Delete selection", bindings: r#"normal "d""#,
	|_ctx| ActionResult::Edit(EditAction::Delete { yank: true }));

bound_action!(delete_no_yank, description: "Delete selection (no yank)", bindings: r#"normal "alt-d""#,
	|_ctx| ActionResult::Edit(EditAction::Delete { yank: false }));

bound_action!(change, description: "Change selection", bindings: r#"normal "c""#,
	|_ctx| ActionResult::Edit(EditAction::Change { yank: true }));

bound_action!(change_no_yank, description: "Change selection (no yank)", bindings: r#"normal "alt-c""#,
	|_ctx| ActionResult::Edit(EditAction::Change { yank: false }));

bound_action!(yank, description: "Yank selection", bindings: r#"normal "y""#,
	|_ctx| ActionResult::Edit(EditAction::Yank));

bound_action!(paste_after, description: "Paste after cursor", bindings: r#"normal "p""#,
	|_ctx| ActionResult::Edit(EditAction::Paste { before: false }));

bound_action!(paste_before, description: "Paste before cursor", bindings: r#"normal "P""#,
	|_ctx| ActionResult::Edit(EditAction::Paste { before: true }));

bound_action!(paste_all_after, description: "Paste all after", bindings: r#"normal "alt-p""#,
	|_ctx| ActionResult::Edit(EditAction::PasteAll { before: false }));

bound_action!(paste_all_before, description: "Paste all before", bindings: r#"normal "alt-P""#,
	|_ctx| ActionResult::Edit(EditAction::PasteAll { before: true }));

bound_action!(undo, description: "Undo last change", bindings: r#"normal "u""#,
	|_ctx| ActionResult::Edit(EditAction::Undo));

bound_action!(redo, description: "Redo last change", bindings: r#"normal "U""#,
	|_ctx| ActionResult::Edit(EditAction::Redo));

bound_action!(indent, description: "Indent selection", bindings: r#"normal ">""#,
	|_ctx| ActionResult::Edit(EditAction::Indent));

bound_action!(deindent, description: "Deindent selection", bindings: r#"normal "<""#,
	|_ctx| ActionResult::Edit(EditAction::Deindent));

bound_action!(to_lowercase, description: "Convert to lowercase", bindings: r#"normal "`""#,
	|_ctx| ActionResult::Edit(EditAction::ToLowerCase));

bound_action!(to_uppercase, description: "Convert to uppercase", bindings: r#"normal "~""#,
	|_ctx| ActionResult::Edit(EditAction::ToUpperCase));

bound_action!(swap_case, description: "Swap case", bindings: r#"normal "alt-`""#,
	|_ctx| ActionResult::Edit(EditAction::SwapCase));

bound_action!(join_lines, description: "Join lines", bindings: r#"normal "alt-j""#,
	|_ctx| ActionResult::Edit(EditAction::JoinLines));

bound_action!(open_below, description: "Open line below", bindings: r#"normal "o""#,
	|_ctx| ActionResult::Edit(EditAction::OpenBelow));

bound_action!(open_above, description: "Open line above", bindings: r#"normal "O""#,
	|_ctx| ActionResult::Edit(EditAction::OpenAbove));

#[distributed_slice(ACTIONS)]
static ACTION_DELETE_BACK: ActionDef = ActionDef {
	id: concat!(env!("CARGO_PKG_NAME"), "::", "delete_back"),
	name: "delete_back",
	aliases: &[],
	description: "Delete character before cursor",
	handler: |_ctx| ActionResult::Edit(EditAction::DeleteBack),
	priority: 0,
	source: evildoer_manifest::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
	required_caps: &[],
	flags: evildoer_manifest::flags::NONE,
};

bound_action!(
	replace_char,
	description: "Replace selection with character",
	bindings: r#"normal "r""#,
	|ctx| match ctx.args.char {
		Some(ch) => ActionResult::Edit(EditAction::ReplaceWithChar { ch }),
		None => ActionResult::Pending(PendingAction {
			kind: PendingKind::ReplaceChar,
			prompt: "replace".into(),
		}),
	}
);
