use xeno_editor::render_api::CompletionKind;

use super::*;

#[test]
fn completion_row_parts_marks_selected_item() {
	let plan = CompletionRenderPlan::new(
		vec![
			CompletionRenderItem::new(String::from("alpha"), CompletionKind::Command, Some(String::from("left")), None, false, false),
			CompletionRenderItem::new(String::from("beta"), CompletionKind::Command, Some(String::from("right")), None, true, false),
		],
		8,
		40,
		true,
		true,
	);

	let parts = completion_row_parts(&plan, &plan.items()[1]);
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
	let selected = SnippetChoiceRenderItem::new(String::from("choice-a"), true);
	let normal = SnippetChoiceRenderItem::new(String::from("choice-b"), false);

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
