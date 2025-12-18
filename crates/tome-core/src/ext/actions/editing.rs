//! Editing actions (delete, yank, paste, undo, redo, etc.).

use linkme::distributed_slice;

use crate::ext::actions::{
	ACTIONS, ActionDef, ActionResult, EditAction, PendingAction, PendingKind,
};

macro_rules! edit_action {
	($static_name:ident, $action_name:expr, $description:expr, $edit:expr) => {
		#[distributed_slice(ACTIONS)]
		static $static_name: ActionDef = ActionDef {
			name: $action_name,
			description: $description,
			handler: |_ctx| ActionResult::Edit($edit),
		};
	};
}

edit_action!(
	ACTION_DELETE,
	"delete",
	"Delete selection",
	EditAction::Delete { yank: true }
);
edit_action!(
	ACTION_DELETE_NO_YANK,
	"delete_no_yank",
	"Delete selection (no yank)",
	EditAction::Delete { yank: false }
);
edit_action!(
	ACTION_CHANGE,
	"change",
	"Change selection",
	EditAction::Change { yank: true }
);
edit_action!(
	ACTION_CHANGE_NO_YANK,
	"change_no_yank",
	"Change selection (no yank)",
	EditAction::Change { yank: false }
);
edit_action!(ACTION_YANK, "yank", "Yank selection", EditAction::Yank);
edit_action!(
	ACTION_PASTE_AFTER,
	"paste_after",
	"Paste after cursor",
	EditAction::Paste { before: false }
);
edit_action!(
	ACTION_PASTE_BEFORE,
	"paste_before",
	"Paste before cursor",
	EditAction::Paste { before: true }
);
edit_action!(
	ACTION_PASTE_ALL_AFTER,
	"paste_all_after",
	"Paste all after",
	EditAction::PasteAll { before: false }
);
edit_action!(
	ACTION_PASTE_ALL_BEFORE,
	"paste_all_before",
	"Paste all before",
	EditAction::PasteAll { before: true }
);
edit_action!(ACTION_UNDO, "undo", "Undo last change", EditAction::Undo);
edit_action!(ACTION_REDO, "redo", "Redo last change", EditAction::Redo);
edit_action!(
	ACTION_INDENT,
	"indent",
	"Indent selection",
	EditAction::Indent
);
edit_action!(
	ACTION_DEINDENT,
	"deindent",
	"Deindent selection",
	EditAction::Deindent
);
edit_action!(
	ACTION_TO_LOWERCASE,
	"to_lowercase",
	"Convert to lowercase",
	EditAction::ToLowerCase
);
edit_action!(
	ACTION_TO_UPPERCASE,
	"to_uppercase",
	"Convert to uppercase",
	EditAction::ToUpperCase
);
edit_action!(
	ACTION_SWAP_CASE,
	"swap_case",
	"Swap case",
	EditAction::SwapCase
);
edit_action!(
	ACTION_JOIN_LINES,
	"join_lines",
	"Join lines",
	EditAction::JoinLines
);
edit_action!(
	ACTION_DELETE_BACK,
	"delete_back",
	"Delete character before cursor",
	EditAction::DeleteBack
);
edit_action!(
	ACTION_OPEN_BELOW,
	"open_below",
	"Open line below",
	EditAction::OpenBelow
);
edit_action!(
	ACTION_OPEN_ABOVE,
	"open_above",
	"Open line above",
	EditAction::OpenAbove
);

#[distributed_slice(ACTIONS)]
static ACTION_REPLACE_CHAR: ActionDef = ActionDef {
	name: "replace_char",
	description: "Replace selection with character",
	handler: |ctx| match ctx.args.char {
		Some(ch) => ActionResult::Edit(EditAction::ReplaceWithChar { ch }),
		None => ActionResult::Pending(PendingAction {
			kind: PendingKind::ReplaceChar,
			prompt: "replace".into(),
		}),
	},
};
