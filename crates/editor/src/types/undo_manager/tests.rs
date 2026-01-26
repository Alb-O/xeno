use std::collections::HashMap;

use xeno_primitives::range::CharIdx;
use xeno_primitives::{EditOrigin, Selection, UndoPolicy};

use super::*;

struct TestHost {
	buffer_id: ViewId,
	doc_id: DocumentId,
}

impl TestHost {
	fn new() -> Self {
		Self {
			buffer_id: ViewId(1),
			doc_id: DocumentId(1),
		}
	}

	fn snapshot(&self) -> ViewSnapshot {
		ViewSnapshot {
			cursor: CharIdx::from(0usize),
			selection: Selection::point(CharIdx::from(0usize)),
			scroll_line: 0,
			scroll_segment: 0,
		}
	}
}

impl UndoHost for TestHost {
	fn guard_readonly(&mut self) -> bool {
		true
	}

	fn doc_id_for_buffer(&self, _buffer_id: ViewId) -> DocumentId {
		self.doc_id
	}

	fn collect_view_snapshots(&self, doc_id: DocumentId) -> HashMap<ViewId, ViewSnapshot> {
		if doc_id == self.doc_id {
			HashMap::from([(self.buffer_id, self.snapshot())])
		} else {
			HashMap::new()
		}
	}

	fn capture_current_view_snapshots(
		&self,
		doc_ids: &[DocumentId],
	) -> HashMap<ViewId, ViewSnapshot> {
		if doc_ids.contains(&self.doc_id) {
			HashMap::from([(self.buffer_id, self.snapshot())])
		} else {
			HashMap::new()
		}
	}

	fn restore_view_snapshots(&mut self, _snapshots: &HashMap<ViewId, ViewSnapshot>) {}

	fn undo_documents(&mut self, _doc_ids: &[DocumentId]) -> bool {
		true
	}

	fn redo_documents(&mut self, _doc_ids: &[DocumentId]) -> bool {
		true
	}

	fn notify_undo(&mut self) {}

	fn notify_redo(&mut self) {}

	fn notify_nothing_to_undo(&mut self) {}

	fn notify_nothing_to_redo(&mut self) {}
}

#[test]
fn with_edit_pushes_group_on_apply() {
	let mut manager = UndoManager::new();
	let mut host = TestHost::new();
	let buffer_id = host.buffer_id;

	let applied = manager.with_edit(
		&mut host,
		buffer_id,
		UndoPolicy::Record,
		EditOrigin::Internal("test"),
		|_host| CommitResult::stub(0),
	);

	assert!(applied);
	assert_eq!(manager.undo_len(), 1);
	assert_eq!(manager.redo_len(), 0);
	assert_eq!(manager.finalize_calls, 1);
}

#[test]
fn with_edit_calls_finalize_on_failure() {
	let mut manager = UndoManager::new();
	let mut host = TestHost::new();
	let buffer_id = host.buffer_id;

	let applied = manager.with_edit(
		&mut host,
		buffer_id,
		UndoPolicy::Record,
		EditOrigin::Internal("test"),
		|_host| CommitResult::blocked(0, false),
	);

	assert!(!applied);
	assert_eq!(manager.undo_len(), 0);
	assert_eq!(manager.redo_len(), 0);
	assert_eq!(manager.finalize_calls, 1);
}
