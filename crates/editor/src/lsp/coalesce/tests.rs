use xeno_primitives::lsp::{LspDocumentChange, LspPosition, LspRange};

use super::coalesce_changes;

fn pos(line: u32, character: u32) -> LspPosition {
	LspPosition::new(line, character)
}

fn point(line: u32, character: u32) -> LspRange {
	LspRange::point(pos(line, character))
}

fn range(sl: u32, sc: u32, el: u32, ec: u32) -> LspRange {
	LspRange::new(pos(sl, sc), pos(el, ec))
}

#[test]
fn no_changes() {
	let changes = vec![];
	assert_eq!(coalesce_changes(changes), vec![]);
}

#[test]
fn single_change() {
	let changes = vec![LspDocumentChange {
		range: point(0, 0),
		new_text: "hello".to_string(),
	}];
	let result = coalesce_changes(changes.clone());
	assert_eq!(result, changes);
}

#[test]
fn consecutive_inserts_merged() {
	let changes = vec![
		LspDocumentChange {
			range: point(1, 5),
			new_text: "foo".to_string(),
		},
		LspDocumentChange {
			range: point(1, 8), // 5 + 3 = 8
			new_text: "bar".to_string(),
		},
	];
	let result = coalesce_changes(changes);
	assert_eq!(result.len(), 1);
	assert_eq!(result[0].range, point(1, 5));
	assert_eq!(result[0].new_text, "foobar");
}

#[test]
fn non_consecutive_inserts_not_merged() {
	let changes = vec![
		LspDocumentChange {
			range: point(1, 5),
			new_text: "foo".to_string(),
		},
		LspDocumentChange {
			range: point(1, 10), // gap between 8 and 10
			new_text: "bar".to_string(),
		},
	];
	let result = coalesce_changes(changes.clone());
	assert_eq!(result.len(), 2);
}

#[test]
fn delete_plus_insert_becomes_replace() {
	let changes = vec![
		LspDocumentChange {
			range: range(1, 5, 1, 10),
			new_text: String::new(),
		},
		LspDocumentChange {
			range: point(1, 5),
			new_text: "new".to_string(),
		},
	];
	let result = coalesce_changes(changes);
	assert_eq!(result.len(), 1);
	assert_eq!(result[0].range, range(1, 5, 1, 10));
	assert_eq!(result[0].new_text, "new");
}

#[test]
fn consecutive_deletes_merged() {
	let changes = vec![
		LspDocumentChange {
			range: range(1, 5, 1, 10),
			new_text: String::new(),
		},
		LspDocumentChange {
			range: range(1, 5, 1, 8), // deletes 3 more chars at same position
			new_text: String::new(),
		},
	];
	let result = coalesce_changes(changes);
	assert_eq!(result.len(), 1);
	assert_eq!(result[0].range, range(1, 5, 1, 13)); // 5 + 5 + 3 = 13
	assert_eq!(result[0].new_text, "");
}

#[test]
fn insert_then_complete_delete_cancels() {
	let changes = vec![
		LspDocumentChange {
			range: point(1, 5),
			new_text: "foo".to_string(),
		},
		LspDocumentChange {
			range: range(1, 5, 1, 8), // delete exactly "foo"
			new_text: String::new(),
		},
	];
	let result = coalesce_changes(changes);
	assert_eq!(result.len(), 1);
	assert_eq!(result[0].range, point(1, 5));
	assert_eq!(result[0].new_text, "");
}

#[test]
fn insert_then_partial_delete() {
	let changes = vec![
		LspDocumentChange {
			range: point(1, 5),
			new_text: "foobar".to_string(),
		},
		LspDocumentChange {
			range: range(1, 5, 1, 8), // delete "foo", keep "bar"
			new_text: String::new(),
		},
	];
	let result = coalesce_changes(changes);
	assert_eq!(result.len(), 1);
	assert_eq!(result[0].range, point(1, 5));
	assert_eq!(result[0].new_text, "bar");
}

#[test]
fn three_consecutive_inserts() {
	let changes = vec![
		LspDocumentChange {
			range: point(0, 0),
			new_text: "a".to_string(),
		},
		LspDocumentChange {
			range: point(0, 1),
			new_text: "b".to_string(),
		},
		LspDocumentChange {
			range: point(0, 2),
			new_text: "c".to_string(),
		},
	];
	let result = coalesce_changes(changes);
	assert_eq!(result.len(), 1);
	assert_eq!(result[0].new_text, "abc");
}

#[test]
fn insert_with_newline() {
	let changes = vec![
		LspDocumentChange {
			range: point(0, 5),
			new_text: "foo\n".to_string(),
		},
		LspDocumentChange {
			range: point(1, 0), // after newline, we're at line 1, col 0
			new_text: "bar".to_string(),
		},
	];
	let result = coalesce_changes(changes);
	assert_eq!(result.len(), 1);
	assert_eq!(result[0].new_text, "foo\nbar");
}
