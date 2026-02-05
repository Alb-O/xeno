use xeno_primitives::UndoPolicy;
use xeno_primitives::transaction::Change;
use xeno_primitives::{Rope, Transaction};

use super::{MAX_UNDO, TxnUndoStore, UndoBackend};

fn test_rope() -> Rope {
	Rope::from("hello world")
}

#[test]
fn txn_store_basic_undo() {
	let mut store = TxnUndoStore::new();
	let original = test_rope();
	let mut content = original.clone();
	let tx = Transaction::change(
		original.slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("goodbye".into()),
		}],
	);
	let undo_tx = tx.invert(&original);

	store.record_transaction(tx.clone(), undo_tx, false);

	tx.apply(&mut content);
	assert_eq!(content.to_string(), "goodbye world");

	let _ = store.undo(&mut content).expect("should have undo");

	assert_eq!(content.to_string(), "hello world");
}

#[test]
fn txn_store_undo_redo_cycle() {
	let mut store = TxnUndoStore::new();
	let original = test_rope();
	let mut content;

	let tx = Transaction::change(
		original.slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("goodbye".into()),
		}],
	);
	let undo_tx = tx.invert(&original);

	store.record_transaction(tx, undo_tx, false);

	content = Rope::from("goodbye world");

	let _ = store.undo(&mut content).expect("should undo");
	assert_eq!(content.to_string(), "hello world");

	let _ = store.redo(&mut content).expect("should redo");
	assert_eq!(content.to_string(), "goodbye world");
}

#[test]
fn txn_store_max_undo_limit() {
	let mut store = TxnUndoStore::new();

	for i in 0..=MAX_UNDO + 10 {
		let rope = Rope::from(format!("version {}", i));
		let tx = Transaction::new(rope.slice(..));
		let undo_tx = tx.invert(&rope);
		store.record_transaction(tx, undo_tx, false);
	}

	assert_eq!(store.undo_len(), MAX_UNDO);
}

#[test]
fn txn_store_grouping_undo() {
	let mut store = TxnUndoStore::new();
	let original = test_rope();
	let mut content = original.clone();

	// First edit
	let tx1 = Transaction::change(
		content.slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("hi".into()),
		}],
	);
	let undo1 = tx1.invert(&content);
	store.record_transaction(tx1.clone(), undo1, false);
	tx1.apply(&mut content);

	// Second edit (merged)
	let tx2 = Transaction::change(
		content.slice(..),
		[Change {
			start: 2,
			end: 2,
			replacement: Some("!".into()),
		}],
	);
	let undo2 = tx2.invert(&content);
	store.record_transaction(tx2.clone(), undo2, true);
	tx2.apply(&mut content);

	assert_eq!(content.to_string(), "hi! world");
	assert_eq!(store.undo_len(), 1);

	// Undo both at once
	let _ = store.undo(&mut content).expect("should undo group");
	assert_eq!(content.to_string(), "hello world");

	// Redo both at once
	let _ = store.redo(&mut content).expect("should redo group");
	assert_eq!(content.to_string(), "hi! world");
}

#[test]
fn backend_undo_redo() {
	let mut backend = UndoBackend::new();
	let mut content = test_rope();
	let mut version;

	let tx = Transaction::change(
		content.slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("goodbye".into()),
		}],
	);

	backend.record_commit(&tx, &content, UndoPolicy::Record, None);
	tx.apply(&mut content);
	version = 1;

	assert_eq!(content.to_string(), "goodbye world");

	let undone = backend.undo(&mut content, &mut version);
	assert!(undone.is_some());
	assert_eq!(content.to_string(), "hello world");
	assert_eq!(version, 2);

	let redone = backend.redo(&mut content, &mut version);
	assert!(redone.is_some());
	assert_eq!(content.to_string(), "goodbye world");
	assert_eq!(version, 3);
}
