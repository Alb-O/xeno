//! Miscellaneous actions: add lines, and stub actions for unimplemented features.

use linkme::distributed_slice;
use tome_base::key::Key;
use tome_manifest::ACTIONS;
use tome_manifest::actions::{ActionDef, ActionResult};
use tome_manifest::editor_ctx::{HandleOutcome, ResultHandler};
use tome_manifest::keybindings::{BindingMode, KEYBINDINGS_NORMAL, KeyBindingDef};

use super::EditAction;
use crate::{NotifyWARNExt, action};

action!(
	add_line_below,
	{ description: "Add empty line below cursor" },
	result: ActionResult::Edit(EditAction::AddLineBelow)
);

action!(
	add_line_above,
	{ description: "Add empty line above cursor" },
	result: ActionResult::Edit(EditAction::AddLineAbove)
);

action!(
	use_selection_as_search,
	{ description: "Use current selection as search pattern" },
	result: ActionResult::UseSelectionAsSearch
);

/// Registers an unimplemented action with keybinding and stub handler.
macro_rules! stub_action {
	($name:ident, key: $key:expr, description: $desc:expr, result: $result:ident, slice: $slice:ident) => {
		paste::paste! {
			fn [<handler_ $name>](_ctx: &tome_manifest::actions::ActionContext) -> ActionResult {
				ActionResult::$result
			}

			#[distributed_slice(ACTIONS)]
			static [<ACTION_ $name:upper>]: ActionDef = ActionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: &[],
				description: $desc,
				handler: [<handler_ $name>],
				priority: 0,
				source: tome_manifest::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				required_caps: &[],
				flags: tome_manifest::flags::NONE,
			};

			#[distributed_slice(KEYBINDINGS_NORMAL)]
			static [<KB_ $name:upper>]: KeyBindingDef = KeyBindingDef {
				mode: BindingMode::Normal,
				key: $key,
				action: stringify!($name),
				priority: 100,
			};

			#[distributed_slice(tome_manifest::actions::$slice)]
			static [<HANDLE_ $name:upper>]: ResultHandler = ResultHandler {
				name: stringify!($name),
				handle: |_, ctx, _| {
					ctx.warn(concat!(stringify!($name), " not yet implemented"));
					HandleOutcome::Handled
				},
			};
		}
	};
}

stub_action!(
	align,
	key: Key::char('&'),
	description: "Align cursors",
	result: Align,
	slice: RESULT_ALIGN_HANDLERS
);

stub_action!(
	copy_indent,
	key: Key::alt('&'),
	description: "Copy indent from previous line",
	result: CopyIndent,
	slice: RESULT_COPY_INDENT_HANDLERS
);

stub_action!(
	tabs_to_spaces,
	key: Key::char('@'),
	description: "Convert tabs to spaces",
	result: TabsToSpaces,
	slice: RESULT_TABS_TO_SPACES_HANDLERS
);

stub_action!(
	spaces_to_tabs,
	key: Key::alt('@'),
	description: "Convert spaces to tabs",
	result: SpacesToTabs,
	slice: RESULT_SPACES_TO_TABS_HANDLERS
);

stub_action!(
	trim_selections,
	key: Key::char('_'),
	description: "Trim whitespace from selections",
	result: TrimSelections,
	slice: RESULT_TRIM_SELECTIONS_HANDLERS
);
