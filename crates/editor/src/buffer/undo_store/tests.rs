use xeno_primitives::transaction::Change;
use xeno_primitives::{Rope, Transaction};
use xeno_runtime_language::LanguageLoader;

use super::{DocumentSnapshot, SnapshotUndoStore, TxnUndoStore, UndoBackend, MAX_UNDO};

fn test_rope() -> Rope {
	Rope::from("hello world")
}

#[test]
fn snapshot_store_basic_undo() {
	let mut store = SnapshotUndoStore::new();
	let original = test_rope();

	store.record_snapshot(DocumentSnapshot {
		rope: original.clone(),
		version: 0,
	});

	let edited = Rope::from("goodbye world");

	let restored = store
		.undo(DocumentSnapshot {
			rope: edited,
			version: 1,
		})
		.expect("should have undo");

	assert_eq!(restored.rope.to_string(), "hello world");
}

#[test]
fn snapshot_store_undo_redo_cycle() {
	let mut store = SnapshotUndoStore::new();
	let original = test_rope();

	store.record_snapshot(DocumentSnapshot {
		rope: original,
		version: 0,
	});

	let edited = Rope::from("goodbye world");

	let restored = store
		.undo(DocumentSnapshot {
			rope: edited.clone(),
			version: 1,
		})
		.expect("should undo");
	assert_eq!(restored.rope.to_string(), "hello world");

	let re_edited = store
		.redo(DocumentSnapshot {
			rope: restored.rope,
			version: 2,
		})
		.expect("should redo");
	assert_eq!(re_edited.rope.to_string(), "goodbye world");
}

#[test]
fn snapshot_store_max_undo_limit() {
	let mut store = SnapshotUndoStore::new();

	for i in 0..=MAX_UNDO + 10 {
		store.record_snapshot(DocumentSnapshot {
			rope: Rope::from(format!("version {}", i)),
			version: i as u64,
		});
	}

	assert_eq!(store.undo_len(), MAX_UNDO);
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

	store.record_transaction(
		tx.clone(),
		&DocumentSnapshot {
			rope: original,
			version: 0,
		},
	);

	tx.apply(&mut content);
	assert_eq!(content.to_string(), "goodbye world");

	let undo_tx = store.undo().expect("should have undo");
	undo_tx.apply(&mut content);
	store.commit_undo();

	assert_eq!(content.to_string(), "hello world");
}

#[test]
fn txn_store_undo_redo_cycle() {
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

	store.record_transaction(
		tx.clone(),
		&DocumentSnapshot {
			rope: original,
			version: 0,
		},
	);
	tx.apply(&mut content);

	let undo_tx = store.undo().expect("should undo");
	undo_tx.apply(&mut content);
	store.commit_undo();
	assert_eq!(content.to_string(), "hello world");

	let redo_tx = store.redo().expect("should redo");
	redo_tx.apply(&mut content);
	store.commit_redo();
	assert_eq!(content.to_string(), "goodbye world");
}

#[test]
fn txn_store_max_undo_limit() {
	let mut store = TxnUndoStore::new();
	let mut content = Rope::from("start");

	for i in 0..=MAX_UNDO + 10 {
		let before = DocumentSnapshot {
			rope: content.clone(),
			version: i as u64,
		};
		let tx = Transaction::change(
			content.slice(..),
			[Change {
				start: 0,
				end: content.len_chars(),
				replacement: Some(format!("version {}", i).into()),
			}],
		);
		store.record_transaction(tx.clone(), &before);
		tx.apply(&mut content);
	}

	assert_eq!(store.undo_len(), MAX_UNDO);
}

#[test]
fn backend_undo_redo_snapshot() {
	let mut backend = UndoBackend::snapshot();
	let mut content = test_rope();
	let mut version = 0u64;
	let loader = LanguageLoader::from_embedded();

	backend.record_commit(
		&Transaction::new(content.slice(..)),
		&DocumentSnapshot {
			rope: content.clone(),
			version,
		},
	);

	content = Rope::from("goodbye world");
	version = 1;

	let undone = backend.undo(&mut content, &mut version, &loader, |_, _| {});
	assert!(undone);
	assert_eq!(content.to_string(), "hello world");
	assert_eq!(version, 2);

	let redone = backend.redo(&mut content, &mut version, &loader, |_, _| {});
	assert!(redone);
	assert_eq!(content.to_string(), "goodbye world");
	assert_eq!(version, 3);
}

#[test]
fn backend_undo_redo_transaction() {
	let mut backend = UndoBackend::transaction();
	let mut content = test_rope();
	let mut version = 0u64;
	let loader = LanguageLoader::from_embedded();

	let tx = Transaction::change(
		content.slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("goodbye".into()),
		}],
	);

	backend.record_commit(
		&tx,
		&DocumentSnapshot {
			rope: content.clone(),
			version,
		},
	);
	tx.apply(&mut content);
	version = 1;

	assert_eq!(content.to_string(), "goodbye world");

	let undone = backend.undo(&mut content, &mut version, &loader, |_, _| {});
	assert!(undone);
	assert_eq!(content.to_string(), "hello world");
	assert_eq!(version, 2);

	let redone = backend.redo(&mut content, &mut version, &loader, |_, _| {});
	assert!(redone);
	assert_eq!(content.to_string(), "goodbye world");
	assert_eq!(version, 3);
}
