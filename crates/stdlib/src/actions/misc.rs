//! Miscellaneous actions: add lines, and stub actions for unimplemented features.

use evildoer_manifest::actions::ActionResult;
use evildoer_manifest::{action, stub_action};

use super::EditAction;

action!(
	add_line_below,
	{ description: "Add empty line below cursor" },
	|_ctx| ActionResult::Edit(EditAction::AddLineBelow)
);

action!(
	add_line_above,
	{ description: "Add empty line above cursor" },
	|_ctx| ActionResult::Edit(EditAction::AddLineAbove)
);

action!(
	use_selection_as_search,
	{ description: "Use current selection as search pattern" },
	|_ctx| ActionResult::UseSelectionAsSearch
);

stub_action!(
	align,
	description: "Align cursors",
	bindings: r#"normal "&""#,
	result: Align,
	handler_slice: RESULT_ALIGN_HANDLERS
);

stub_action!(
	copy_indent,
	description: "Copy indent from previous line",
	bindings: r#"normal "alt-&""#,
	result: CopyIndent,
	handler_slice: RESULT_COPY_INDENT_HANDLERS
);

stub_action!(
	tabs_to_spaces,
	description: "Convert tabs to spaces",
	bindings: r#"normal "@""#,
	result: TabsToSpaces,
	handler_slice: RESULT_TABS_TO_SPACES_HANDLERS
);

stub_action!(
	spaces_to_tabs,
	description: "Convert spaces to tabs",
	bindings: r#"normal "alt-@""#,
	result: SpacesToTabs,
	handler_slice: RESULT_SPACES_TO_TABS_HANDLERS
);

stub_action!(
	trim_selections,
	description: "Trim whitespace from selections",
	bindings: r#"normal "_""#,
	result: TrimSelections,
	handler_slice: RESULT_TRIM_SELECTIONS_HANDLERS
);
