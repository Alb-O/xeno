use super::*;

#[test]
fn completion_row_parts_marks_selected_item() {
	let plan = CompletionRenderPlan {
		max_label_width: 8,
		target_row_width: 40,
		show_kind: true,
		show_right: true,
		items: vec![
			xeno_editor::completion::CompletionRenderItem {
				label: String::from("alpha"),
				kind: xeno_editor::completion::CompletionKind::Command,
				right: Some(String::from("left")),
				match_indices: None,
				selected: false,
				command_alias_match: false,
			},
			xeno_editor::completion::CompletionRenderItem {
				label: String::from("beta"),
				kind: xeno_editor::completion::CompletionKind::Command,
				right: Some(String::from("right")),
				match_indices: None,
				selected: true,
				command_alias_match: false,
			},
		],
	};

	let parts = completion_row_parts(&plan, &plan.items[1]);
	assert_eq!(
		parts,
		CompletionRowParts {
			marker: ">",
			label: String::from("beta"),
			kind: Some(String::from("Command")),
			right: Some(String::from("right")),
		}
	);
}

#[test]
fn snippet_row_parts_prefixes_selected_rows() {
	let selected = SnippetChoiceRenderItem {
		option: String::from("choice-a"),
		selected: true,
	};
	let normal = SnippetChoiceRenderItem {
		option: String::from("choice-b"),
		selected: false,
	};

	assert_eq!(
		snippet_row_parts(&selected),
		SnippetRowParts {
			marker: ">",
			option: String::from("choice-a"),
		}
	);
	assert_eq!(
		snippet_row_parts(&normal),
		SnippetRowParts {
			marker: " ",
			option: String::from("choice-b"),
		}
	);
}
