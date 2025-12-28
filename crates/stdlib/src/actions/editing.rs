//! Editing actions (delete, yank, paste, undo, redo, etc.).

use linkme::distributed_slice;
use evildoer_base::key::Key;
use evildoer_manifest::actions::{ActionDef, ActionResult, EditAction, PendingAction, PendingKind};
use evildoer_manifest::{ACTIONS, bound_action};

macro_rules! bound_edit_action {
	($name:ident, key: $key:expr, description: $desc:expr, edit: $edit:expr) => {
		bound_action!(
			$name,
			description: $desc,
			bindings: [Normal => [$key]],
			|_ctx| ActionResult::Edit($edit)
		);
	};
}

bound_edit_action!(delete, key: Key::char('d'), description: "Delete selection", edit: EditAction::Delete { yank: true });
bound_edit_action!(delete_no_yank, key: Key::alt('d'), description: "Delete selection (no yank)", edit: EditAction::Delete { yank: false });
bound_edit_action!(change, key: Key::char('c'), description: "Change selection", edit: EditAction::Change { yank: true });
bound_edit_action!(change_no_yank, key: Key::alt('c'), description: "Change selection (no yank)", edit: EditAction::Change { yank: false });
bound_edit_action!(yank, key: Key::char('y'), description: "Yank selection", edit: EditAction::Yank);

bound_edit_action!(paste_after, key: Key::char('p'), description: "Paste after cursor", edit: EditAction::Paste { before: false });
bound_edit_action!(paste_before, key: Key::char('P'), description: "Paste before cursor", edit: EditAction::Paste { before: true });
bound_edit_action!(paste_all_after, key: Key::alt('p'), description: "Paste all after", edit: EditAction::PasteAll { before: false });
bound_edit_action!(paste_all_before, key: Key::alt('P'), description: "Paste all before", edit: EditAction::PasteAll { before: true });

bound_edit_action!(undo, key: Key::char('u'), description: "Undo last change", edit: EditAction::Undo);
bound_edit_action!(redo, key: Key::char('U'), description: "Redo last change", edit: EditAction::Redo);

bound_edit_action!(indent, key: Key::char('>'), description: "Indent selection", edit: EditAction::Indent);
bound_edit_action!(deindent, key: Key::char('<'), description: "Deindent selection", edit: EditAction::Deindent);

bound_edit_action!(to_lowercase, key: Key::char('`'), description: "Convert to lowercase", edit: EditAction::ToLowerCase);
bound_edit_action!(to_uppercase, key: Key::char('~'), description: "Convert to uppercase", edit: EditAction::ToUpperCase);
bound_edit_action!(swap_case, key: Key::alt('`'), description: "Swap case", edit: EditAction::SwapCase);

bound_edit_action!(join_lines, key: Key::alt('j'), description: "Join lines", edit: EditAction::JoinLines);
bound_edit_action!(open_below, key: Key::char('o'), description: "Open line below", edit: EditAction::OpenBelow);
bound_edit_action!(open_above, key: Key::char('O'), description: "Open line above", edit: EditAction::OpenAbove);

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
	bindings: [Normal => [Key::char('r')]],
	|ctx| match ctx.args.char {
		Some(ch) => ActionResult::Edit(EditAction::ReplaceWithChar { ch }),
		None => ActionResult::Pending(PendingAction {
			kind: PendingKind::ReplaceChar,
			prompt: "replace".into(),
		}),
	}
);
