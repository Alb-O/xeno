//! Miscellaneous actions: add lines, and stub actions for unimplemented features.

use evildoer_manifest::actions::ActionResult;
use evildoer_manifest::bound_action;
use evildoer_manifest::editor_ctx::{HandleOutcome, ResultHandler};
use linkme::distributed_slice;

use super::EditAction;
use crate::{NotifyWARNExt, action};

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

macro_rules! stub_action {
	($name:ident, description: $desc:expr, bindings: $kdl:literal, result: $result:ident, slice: $slice:ident) => {
		bound_action!(
			$name,
			description: $desc,
			bindings: $kdl,
			|_ctx| ActionResult::$result
		);

		paste::paste! {
			#[distributed_slice(evildoer_manifest::actions::$slice)]
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

stub_action!(align, description: "Align cursors",
	bindings: r#"normal "&""#, result: Align, slice: RESULT_ALIGN_HANDLERS);

stub_action!(copy_indent, description: "Copy indent from previous line",
	bindings: r#"normal "alt-&""#, result: CopyIndent, slice: RESULT_COPY_INDENT_HANDLERS);

stub_action!(tabs_to_spaces, description: "Convert tabs to spaces",
	bindings: r#"normal "@""#, result: TabsToSpaces, slice: RESULT_TABS_TO_SPACES_HANDLERS);

stub_action!(spaces_to_tabs, description: "Convert spaces to tabs",
	bindings: r#"normal "alt-@""#, result: SpacesToTabs, slice: RESULT_SPACES_TO_TABS_HANDLERS);

stub_action!(trim_selections, description: "Trim whitespace from selections",
	bindings: r#"normal "_""#, result: TrimSelections, slice: RESULT_TRIM_SELECTIONS_HANDLERS);
