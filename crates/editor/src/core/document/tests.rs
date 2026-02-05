use xeno_primitives::transaction::Change;
use xeno_primitives::{
	EditCommit, EditError, EditOrigin, SyntaxOutcome, SyntaxPolicy, Transaction, UndoPolicy,
};
use xeno_runtime_language::LanguageLoader;

use super::Document;

fn language_loader() -> LanguageLoader {
	LanguageLoader::from_embedded()
}

fn make_commit(tx: Transaction) -> EditCommit {
	EditCommit {
		tx,
		undo: UndoPolicy::Record,
		syntax: SyntaxPolicy::None,
		origin: EditOrigin::Internal("test"),
		selection_after: None,
	}
}

#[test]
fn commit_readonly_returns_error() {
	let mut doc = Document::new("hello".into(), None);
	doc.set_readonly(true);

	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("world".into()),
		}],
	);

	let result = doc.commit(make_commit(tx), false, &language_loader());
	assert!(matches!(result, Err(EditError::ReadOnly { .. })));
	assert_eq!(doc.content().to_string(), "hello");
}

#[test]
fn commit_increments_version_once() {
	let mut doc = Document::new("hello".into(), None);
	let version_before = doc.version();

	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("world".into()),
		}],
	);

	let result = doc
		.commit(make_commit(tx), false, &language_loader())
		.unwrap();

	assert_eq!(result.version_before, version_before);
	assert_eq!(result.version_after, version_before + 1);
	assert_eq!(doc.version(), version_before + 1);
}

#[test]
fn commit_clears_redo_when_undo_recorded() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	let tx1 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 5,
			end: 5,
			replacement: Some(" world".into()),
		}],
	);
	doc.commit(make_commit(tx1), false, &loader).unwrap();

	let tx2 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 11,
			end: 11,
			replacement: Some("!".into()),
		}],
	);
	doc.commit(make_commit(tx2), false, &loader).unwrap();
	assert!(doc.can_undo());

	doc.undo();
	assert!(doc.can_redo());

	let tx3 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("Hi ".into()),
		}],
	);
	let result = doc.commit(make_commit(tx3), false, &loader).unwrap();
	assert!(result.undo_recorded);
	assert!(!doc.can_redo());
}

#[test]
fn commit_sets_modified_flag() {
	let mut doc = Document::new("hello".into(), None);
	assert!(!doc.is_modified());

	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("X".into()),
		}],
	);
	doc.commit(make_commit(tx), false, &language_loader())
		.unwrap();

	assert!(doc.is_modified());
}

#[test]
fn commit_no_undo_policy_skips_recording_and_clears_history() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	// First, record some history
	let tx1 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 5,
			end: 5,
			replacement: Some("!".into()),
		}],
	);
	doc.commit(make_commit(tx1), false, &loader).unwrap();
	assert!(doc.can_undo());

	// Now apply a NoUndo edit
	let tx2 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("world".into()),
		}],
	);
	let commit = EditCommit {
		tx: tx2,
		undo: UndoPolicy::NoUndo,
		syntax: SyntaxPolicy::None,
		origin: EditOrigin::Internal("test"),
		selection_after: None,
	};

	let result = doc.commit(commit, false, &loader).unwrap();
	assert!(!result.undo_recorded);
	assert!(
		!doc.can_undo(),
		"NoUndo should clear existing history to prevent corruption"
	);
	assert_eq!(doc.content().to_string(), "world!");
}

#[test]
fn commit_merge_policy_groups_inserts() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	let tx1 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 5,
			end: 5,
			replacement: Some("A".into()),
		}],
	);
	let result1 = doc
		.commit(
			EditCommit {
				tx: tx1,
				undo: UndoPolicy::Record,
				syntax: SyntaxPolicy::None,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
			false, // Not merged
			&loader,
		)
		.unwrap();
	assert!(result1.undo_recorded);

	let tx2 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 6,
			end: 6,
			replacement: Some("B".into()),
		}],
	);
	let result2 = doc
		.commit(
			EditCommit {
				tx: tx2,
				undo: UndoPolicy::Record,
				syntax: SyntaxPolicy::None,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
			true, // Merged
			&loader,
		)
		.unwrap();
	assert!(!result2.undo_recorded);

	assert_eq!(doc.content().to_string(), "helloAB");
	assert_eq!(doc.undo_len(), 1);
}

#[test]
fn commit_boundary_policy_breaks_insert_group() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	let tx1 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 5,
			end: 5,
			replacement: Some("A".into()),
		}],
	);
	doc.commit(
		EditCommit {
			tx: tx1,
			undo: UndoPolicy::Record,
			syntax: SyntaxPolicy::None,
			origin: EditOrigin::Internal("test"),
			selection_after: None,
		},
		false,
		&loader,
	)
	.unwrap();

	let tx2 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 6,
			end: 6,
			replacement: Some("B".into()),
		}],
	);
	let result = doc
		.commit(
			EditCommit {
				tx: tx2,
				undo: UndoPolicy::Record,
				syntax: SyntaxPolicy::None,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
			false, // Explicit boundary (merge=false)
			&loader,
		)
		.unwrap();
	assert!(result.undo_recorded);
	assert_eq!(doc.undo_len(), 2);

	let tx3 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 7,
			end: 7,
			replacement: Some("C".into()),
		}],
	);
	let result3 = doc
		.commit(
			EditCommit {
				tx: tx3,
				undo: UndoPolicy::Record,
				syntax: SyntaxPolicy::None,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
			true, // Merge again
			&loader,
		)
		.unwrap();
	assert!(!result3.undo_recorded);
	assert_eq!(doc.undo_len(), 2); // Still 2 steps because it merged!
}

#[test]
fn commit_syntax_mark_dirty() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("X".into()),
		}],
	);
	let result = doc
		.commit(
			EditCommit {
				tx,
				undo: UndoPolicy::Record,
				syntax: SyntaxPolicy::MarkDirty,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
			false,
			&loader,
		)
		.unwrap();

	assert_eq!(result.syntax_outcome, SyntaxOutcome::MarkedDirty);
}

#[test]
fn commit_incremental_or_dirty_without_syntax_marks_dirty() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("X".into()),
		}],
	);
	let result = doc
		.commit(
			EditCommit {
				tx,
				undo: UndoPolicy::Record,
				syntax: SyntaxPolicy::IncrementalOrDirty,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
			false,
			&loader,
		)
		.unwrap();

	assert_eq!(result.syntax_outcome, SyntaxOutcome::MarkedDirty);
}

#[test]
fn reset_content_clears_undo_history() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("X".into()),
		}],
	);
	doc.commit(make_commit(tx), false, &loader).unwrap();
	assert!(doc.can_undo());
	assert_eq!(doc.undo_len(), 1);

	doc.reset_content("reset");
	assert!(!doc.can_undo());
	assert!(!doc.can_redo());
	assert_eq!(doc.undo_len(), 0);
	assert_eq!(doc.redo_len(), 0);
}

#[test]
fn undo_redo_to_clean_state() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();
	assert!(!doc.is_modified());

	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 5,
			end: 5,
			replacement: Some("!".into()),
		}],
	);
	doc.commit(make_commit(tx), false, &loader).unwrap();
	assert!(doc.is_modified());

	doc.undo();
	assert!(
		!doc.is_modified(),
		"Undo to original state should clear modified flag"
	);

	doc.redo();
	assert!(doc.is_modified(), "Redo should set modified flag again");
}

#[test]
fn identity_commit_does_not_bump_version_or_modified() {
	let mut doc = Document::new("hello".into(), None);
	let version_before = doc.version();
	let loader = language_loader();

	let tx = Transaction::new(doc.content().slice(..));
	let result = doc.commit(make_commit(tx), false, &loader).unwrap();

	assert!(!result.applied);
	assert_eq!(doc.version(), version_before);
	assert!(!doc.is_modified());
}
