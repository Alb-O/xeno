//! Miscellaneous actions including unimplemented stubs.
//!
//! Stub actions return their [`ActionResult`] variant but have no registered handler.
//! The dispatch system shows "Unhandled action result" for these until implemented.

use evildoer_manifest::action;
use evildoer_manifest::actions::ActionResult;

use super::EditAction;

action!(add_line_below, { description: "Add empty line below cursor" },
	|_ctx| ActionResult::Edit(EditAction::AddLineBelow));

action!(add_line_above, { description: "Add empty line above cursor" },
	|_ctx| ActionResult::Edit(EditAction::AddLineAbove));

action!(use_selection_as_search, { description: "Use current selection as search pattern" },
	|_ctx| ActionResult::UseSelectionAsSearch);

// TODO: implement handler
action!(align, { description: "Align cursors", bindings: r#"normal "&""# },
	|_ctx| ActionResult::Align);

// TODO: implement handler
action!(copy_indent, { description: "Copy indent from previous line", bindings: r#"normal "alt-&""# },
	|_ctx| ActionResult::CopyIndent);

// TODO: implement handler
action!(tabs_to_spaces, { description: "Convert tabs to spaces", bindings: r#"normal "@""# },
	|_ctx| ActionResult::TabsToSpaces);

// TODO: implement handler
action!(spaces_to_tabs, { description: "Convert spaces to tabs", bindings: r#"normal "alt-@""# },
	|_ctx| ActionResult::SpacesToTabs);

// TODO: implement handler
action!(trim_selections, { description: "Trim whitespace from selections", bindings: r#"normal "_""# },
	|_ctx| ActionResult::TrimSelections);
