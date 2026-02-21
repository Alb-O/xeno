use xeno_primitives::{Change, Selection};

use super::*;

#[test]
fn test_insert_computes_correct_range() {
	let rope = Rope::from("hello\nworld\n");
	let sel = Selection::single(6, 6);
	let tx = Transaction::insert(rope.slice(..), &sel, "beautiful ".to_string());

	let changes = compute_lsp_changes(&rope, &tx, OffsetEncoding::Utf16);
	let IncrementalResult::Incremental(changes) = changes else {
		panic!("expected incremental changes");
	};

	assert_eq!(changes.len(), 1);
	assert_eq!(changes[0].range, LspRange::point(LspPosition::new(1, 0)));
	assert_eq!(changes[0].new_text, "beautiful ");
}

#[test]
fn test_delete_line_computes_correct_range() {
	let rope = Rope::from("line1\nline2\nline3\n");
	// Selection from 6 (start of "line2") to 11 (the \n after "line2").
	// Transaction::delete uses to_inclusive(), so we select up to but not
	// including position 12 to delete exactly "line2\n".
	let sel = Selection::single(6, 11);
	let tx = Transaction::delete(rope.slice(..), &sel);

	let changes = compute_lsp_changes(&rope, &tx, OffsetEncoding::Utf16);
	let IncrementalResult::Incremental(changes) = changes else {
		panic!("expected incremental changes");
	};

	assert_eq!(changes.len(), 1);
	assert_eq!(changes[0].range, LspRange::new(LspPosition::new(1, 0), LspPosition::new(2, 0)));
	assert_eq!(changes[0].new_text, "");
}

#[test]
fn test_multi_cursor_edit() {
	let rope = Rope::from("hello\nworld\n");
	let changes = vec![
		Change {
			start: 0,
			end: 0,
			replacement: Some("\n".to_string()),
		},
		Change {
			start: 6,
			end: 6,
			replacement: Some("X".to_string()),
		},
	];
	let tx = Transaction::change(rope.slice(..), changes);

	let changes = compute_lsp_changes(&rope, &tx, OffsetEncoding::Utf16);
	let IncrementalResult::Incremental(changes) = changes else {
		panic!("expected incremental changes");
	};

	assert_eq!(changes.len(), 2);
	assert_eq!(changes[0].range, LspRange::point(LspPosition::new(0, 0)));
	assert_eq!(changes[0].new_text, "\n");
	assert_eq!(changes[1].range, LspRange::point(LspPosition::new(2, 0)));
	assert_eq!(changes[1].new_text, "X");
}
