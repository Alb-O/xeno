use super::*;

#[test]
fn completion_row_label_marks_selected_item() {
	let plan = CompletionRenderPlan {
		max_label_width: 8,
		target_row_width: 40,
		show_kind: false,
		show_right: false,
		items: vec![
			xeno_editor::completion::CompletionRenderItem {
				label: String::from("alpha"),
				kind: xeno_editor::completion::CompletionKind::Command,
				right: None,
				match_indices: None,
				selected: false,
				command_alias_match: false,
			},
			xeno_editor::completion::CompletionRenderItem {
				label: String::from("beta"),
				kind: xeno_editor::completion::CompletionKind::Command,
				right: None,
				match_indices: None,
				selected: true,
				command_alias_match: false,
			},
		],
	};

	let row = completion_row_label(&plan, &plan.items[1]);
	assert_eq!(row, "> beta");
}

#[test]
fn snippet_row_label_prefixes_selected_rows() {
	let selected = SnippetChoiceRenderItem {
		option: String::from("choice-a"),
		selected: true,
	};
	let normal = SnippetChoiceRenderItem {
		option: String::from("choice-b"),
		selected: false,
	};

	assert_eq!(snippet_row_label(&selected), "> choice-a");
	assert_eq!(snippet_row_label(&normal), "  choice-b");
}
