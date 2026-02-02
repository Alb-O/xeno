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

	let result = doc.commit(make_commit(tx), &language_loader());
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

	let result = doc.commit(make_commit(tx), &language_loader()).unwrap();

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
	doc.commit(make_commit(tx1), &loader).unwrap();

	let tx2 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 11,
			end: 11,
			replacement: Some("!".into()),
		}],
	);
	doc.commit(make_commit(tx2), &loader).unwrap();
	assert!(doc.can_undo());

	doc.undo(&loader);
	assert!(doc.can_redo());

	let tx3 = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("Hi ".into()),
		}],
	);
	let result = doc.commit(make_commit(tx3), &loader).unwrap();
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
	doc.commit(make_commit(tx), &language_loader()).unwrap();

	assert!(doc.is_modified());
}

#[test]
fn commit_no_undo_policy_skips_recording() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: Some("world".into()),
		}],
	);
	let commit = EditCommit {
		tx,
		undo: UndoPolicy::NoUndo,
		syntax: SyntaxPolicy::None,
		origin: EditOrigin::Internal("test"),
		selection_after: None,
	};

	let result = doc.commit(commit, &loader).unwrap();
	assert!(!result.undo_recorded);
	assert!(!doc.can_undo());
	assert_eq!(doc.content().to_string(), "world");
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
				undo: UndoPolicy::MergeWithCurrentGroup,
				syntax: SyntaxPolicy::None,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
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
				undo: UndoPolicy::MergeWithCurrentGroup,
				syntax: SyntaxPolicy::None,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
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
			undo: UndoPolicy::MergeWithCurrentGroup,
			syntax: SyntaxPolicy::None,
			origin: EditOrigin::Internal("test"),
			selection_after: None,
		},
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
				undo: UndoPolicy::Boundary,
				syntax: SyntaxPolicy::None,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
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
				undo: UndoPolicy::MergeWithCurrentGroup,
				syntax: SyntaxPolicy::None,
				origin: EditOrigin::Internal("test"),
				selection_after: None,
			},
			&loader,
		)
		.unwrap();
	assert!(result3.undo_recorded);
	assert_eq!(doc.undo_len(), 3);
}

#[test]
fn commit_syntax_mark_dirty() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	assert!(!doc.is_syntax_dirty());

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
			&loader,
		)
		.unwrap();

	assert!(doc.is_syntax_dirty());
	assert_eq!(result.syntax_outcome, SyntaxOutcome::MarkedDirty);
}

#[test]
fn commit_incremental_or_dirty_without_syntax_marks_dirty() {
	let mut doc = Document::new("hello".into(), None);
	let loader = language_loader();

	assert!(!doc.has_syntax());
	assert!(!doc.is_syntax_dirty());

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
			&loader,
		)
		.unwrap();

	assert!(doc.is_syntax_dirty());
	assert_eq!(result.syntax_outcome, SyntaxOutcome::MarkedDirty);
}

#[test]
fn reset_content_marks_syntax_dirty_and_reparses() {
	use xeno_runtime_language::syntax::Syntax;

	let mut doc = Document::new("fn main() {}".into(), None);
	let loader = language_loader();

	// init_syntax_for_language sets language_id (syntax creation is deferred to SyntaxManager)
	doc.init_syntax_for_language("rust", &loader);
	assert!(doc.language_id().is_some());
	assert!(!doc.has_syntax());
	assert!(!doc.is_syntax_dirty());

	// Create syntax directly (simulating what SyntaxManager would do)
	let syntax = Syntax::new(
		doc.content().slice(..),
		doc.language_id().unwrap(),
		&loader,
		xeno_runtime_language::SyntaxOptions::default(),
	)
	.ok();
	doc.set_syntax(syntax);
	assert!(doc.has_syntax());
	assert!(!doc.is_syntax_dirty());

	// reset_content marks syntax as dirty and drops it
	doc.reset_content("let x = 1;");
	assert!(doc.is_syntax_dirty());
	assert!(!doc.has_syntax());
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
	doc.commit(make_commit(tx), &loader).unwrap();
	assert!(doc.can_undo());
	assert_eq!(doc.undo_len(), 1);

	doc.reset_content("reset");
	assert!(!doc.can_undo());
	assert!(!doc.can_redo());
	assert_eq!(doc.undo_len(), 0);
	assert_eq!(doc.redo_len(), 0);
}

#[test]
fn reset_content_bumps_version_and_marks_dirty_without_syntax() {
	let mut doc = Document::new("hello".into(), None);
	let v0 = doc.version();

	assert!(!doc.is_syntax_dirty());

	doc.reset_content("reset");

	assert!(doc.is_syntax_dirty());
	assert_ne!(doc.version(), v0);
}

#[test]
fn test_undo_redo_bumps_syntax_version() {
	use xeno_runtime_language::syntax::Syntax;

	let mut doc = Document::new("fn main() {}".into(), None);
	let loader = language_loader();
	doc.init_syntax_for_language("rust", &loader);

	// Initial version bump from init_syntax
	let v0 = doc.syntax_version;
	assert!(v0 > 0);

	// Install syntax
	let syntax = Syntax::new(
		doc.content().slice(..),
		doc.language_id().unwrap(),
		&loader,
		xeno_runtime_language::SyntaxOptions::default(),
	)
	.unwrap();
	doc.set_syntax(Some(syntax));
	let v1 = doc.syntax_version;
	assert!(v1 > v0);

	// Edit
	let tx = Transaction::change(
		doc.content().slice(..),
		[Change {
			start: 11,
			end: 11,
			replacement: Some("\n".into()),
		}],
	);
	doc.commit(
		EditCommit {
			tx,
			undo: UndoPolicy::Record,
			syntax: SyntaxPolicy::IncrementalOrDirty,
			origin: EditOrigin::Internal("test"),
			selection_after: None,
		},
		&loader,
	)
	.unwrap();
	let v2 = doc.syntax_version;
	assert!(v2 > v1); // Incremental update bumps version

	// Undo
	doc.undo(&loader);
	let v3 = doc.syntax_version;
	assert!(v3 > v2); // Undo bumps version

	// Redo
	doc.redo(&loader);
	let v4 = doc.syntax_version;
	assert!(v4 > v3); // Redo bumps version
}
