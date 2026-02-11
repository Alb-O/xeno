use xeno_lsp::lsp_types;

use super::*;

#[test]
fn workspace_edit_plan_manual_construct() {
	let plan = WorkspaceEditPlan { per_buffer: Vec::new() };
	assert!(plan.affected_buffer_ids().is_empty());
}

#[test]
fn coalesce_rejects_overlap() {
	let mut edits = vec![
		PlannedTextEdit {
			range: 0..2,
			replacement: "a".into(),
		},
		PlannedTextEdit {
			range: 1..3,
			replacement: "b".into(),
		},
	];
	let uri: Uri = "file:///tmp/test.rs".parse().unwrap();
	let err = coalesce_and_validate(&mut edits, &uri).unwrap_err();
	assert!(matches!(err, ApplyError::OverlappingEdits(_)));
}

#[test]
fn convert_text_edit_utf16() {
	let rope = xeno_primitives::Rope::from("a😀b\n");
	let edit = TextEdit {
		range: lsp_types::Range {
			start: lsp_types::Position { line: 0, character: 1 },
			end: lsp_types::Position { line: 0, character: 3 },
		},
		new_text: "X".into(),
	};
	let planned = convert_text_edit(&rope, OffsetEncoding::Utf16, &edit).unwrap();
	assert_eq!(planned.range.start, 1);
	assert_eq!(planned.range.end, 2);
}
