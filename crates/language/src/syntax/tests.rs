use xeno_primitives::{Rope, Transaction};

use super::*;

#[test]
fn test_generate_edits_insert() {
	use xeno_primitives::Change;
	let doc = Rope::from("hello world");
	let changes = vec![Change {
		start: 5,
		end: 5,
		replacement: Some(" beautiful".into()),
	}];
	let tx = Transaction::change(doc.slice(..), changes);

	let edits = generate_edits(doc.slice(..), tx.changes());
	assert_eq!(edits.len(), 1);
	assert_eq!(edits[0].start_byte, 5);
	assert_eq!(edits[0].old_end_byte, 5);
	assert_eq!(edits[0].new_end_byte, 15);
}

#[test]
fn test_generate_edits_delete() {
	use xeno_primitives::Change;
	let doc = Rope::from("hello world");
	let changes = vec![Change {
		start: 5,
		end: 11,
		replacement: None,
	}];
	let tx = Transaction::change(doc.slice(..), changes);

	let edits = generate_edits(doc.slice(..), tx.changes());
	assert_eq!(edits.len(), 1);
	assert_eq!(edits[0].start_byte, 5);
	assert_eq!(edits[0].old_end_byte, 11);
	assert_eq!(edits[0].new_end_byte, 5);
}

#[test]
fn test_generate_edits_replace() {
	use xeno_primitives::Change;
	let doc = Rope::from("hello world");
	let changes = vec![Change {
		start: 6,
		end: 11,
		replacement: Some("rust".into()),
	}];
	let tx = Transaction::change(doc.slice(..), changes);

	let edits = generate_edits(doc.slice(..), tx.changes());
	assert_eq!(edits.len(), 1);
	assert_eq!(edits[0].start_byte, 6);
	assert_eq!(edits[0].old_end_byte, 11);
	assert_eq!(edits[0].new_end_byte, 10);
}

#[test]
fn test_generate_edits_multi_insert_requires_coordinate_shift() {
	use xeno_primitives::Change;

	// ASCII => bytes == chars for simple assertions.
	let doc = Rope::from("hello world"); // len_bytes = 11

	// Two inserts in one ChangeSet: at start and at end (in original coordinates).
	let changes = vec![
		Change {
			start: 0,
			end: 0,
			replacement: Some("X".into()),
		},
		Change {
			start: 11,
			end: 11,
			replacement: Some("Y".into()),
		},
	];

	let tx = Transaction::change(doc.slice(..), changes);
	let edits = generate_edits(doc.slice(..), tx.changes());

	assert_eq!(edits.len(), 2);

	// First insert at 0 is fine.
	assert_eq!(edits[0].start_byte, 0);
	assert_eq!(edits[0].old_end_byte, 0);
	assert_eq!(edits[0].new_end_byte, 1);

	// If InputEdits are applied sequentially (Tree::edit style),
	// the second insert's coordinates must be shifted by +1 byte due to the prior insert.
	assert_eq!(edits[1].start_byte, 12, "start_byte should be shifted");
	assert_eq!(edits[1].old_end_byte, 12, "old_end_byte should be shifted");
	assert_eq!(edits[1].new_end_byte, 13, "new_end_byte should be shifted");

	// Bonus: Point.col is also in bytes; should match shifted coordinates on row 0.
	assert_eq!(edits[1].start_point.row, 0);
	assert_eq!(edits[1].start_point.col, 12);
	assert_eq!(edits[1].new_end_point.col, 13);
}
