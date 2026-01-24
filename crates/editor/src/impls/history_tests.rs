//! Behavior-lock tests for editor-level undo/redo.

use proptest::prelude::*;
use xeno_primitives::range::CharIdx;
use xeno_primitives::transaction::Change;
use xeno_primitives::{EditOrigin, Selection, Transaction, UndoPolicy};

use super::Editor;
use crate::buffer::ViewId;

fn test_editor(content: &str) -> Editor {
	Editor::from_content(content.to_string(), None)
}

fn apply_test_edit(editor: &mut Editor, text: &str, at: usize) -> bool {
	let buffer_id = editor.focused_view();
	let tx = {
		let buffer = editor.state.core.buffers.focused_buffer();
		let rope = buffer.with_doc(|doc| doc.content().clone());
		Transaction::change(
			rope.slice(..),
			[Change {
				start: at,
				end: at,
				replacement: Some(text.into()),
			}],
		)
	};
	editor.apply_edit(
		buffer_id,
		&tx,
		None,
		UndoPolicy::Record,
		EditOrigin::Internal("test"),
	)
}

fn set_cursor(editor: &mut Editor, pos: usize) {
	let buffer = editor.state.core.buffers.focused_buffer_mut();
	buffer.cursor = CharIdx::from(pos);
	buffer.selection = Selection::point(CharIdx::from(pos));
}

fn set_scroll(editor: &mut Editor, line: usize, segment: usize) {
	let buffer = editor.state.core.buffers.focused_buffer_mut();
	buffer.scroll_line = line;
	buffer.scroll_segment = segment;
}

fn get_cursor(editor: &Editor, buffer_id: ViewId) -> usize {
	editor
		.state
		.core
		.buffers
		.get_buffer(buffer_id)
		.unwrap()
		.cursor
		.into()
}

fn get_scroll(editor: &Editor, buffer_id: ViewId) -> (usize, usize) {
	let buffer = editor.state.core.buffers.get_buffer(buffer_id).unwrap();
	(buffer.scroll_line, buffer.scroll_segment)
}

#[test]
fn undo_restores_cursor_position() {
	let mut editor = test_editor("hello world");
	set_cursor(&mut editor, 5);

	apply_test_edit(&mut editor, " there", 5);

	assert_eq!(editor.state.core.undo_manager.undo_len(), 1);

	editor.undo();

	let cursor = get_cursor(&editor, editor.focused_view());
	assert_eq!(cursor, 5, "undo should restore cursor to pre-edit position");
}

#[test]
fn undo_restores_scroll_position() {
	let mut editor = test_editor("line1\nline2\nline3\nline4\nline5");
	set_scroll(&mut editor, 2, 0);

	apply_test_edit(&mut editor, "X", 0);

	editor.undo();

	let (scroll_line, scroll_segment) = get_scroll(&editor, editor.focused_view());
	assert_eq!(scroll_line, 2, "undo should restore scroll_line");
	assert_eq!(scroll_segment, 0, "undo should restore scroll_segment");
}

#[test]
fn undo_restores_view_state_for_multiple_buffers_same_document() {
	let mut editor = test_editor("shared document content");

	let buffer1_id = editor.focused_view();
	set_cursor(&mut editor, 7);
	set_scroll(&mut editor, 0, 0);

	let buffer2_id = editor.state.core.buffers.clone_focused_buffer_for_split();
	editor.state.core.buffers.set_focused_view(buffer2_id);
	set_cursor(&mut editor, 15);
	set_scroll(&mut editor, 1, 0);

	apply_test_edit(&mut editor, "X", 0);

	assert_eq!(editor.state.core.undo_manager.undo_len(), 1);
	let group = editor
		.state
		.core
		.undo_manager
		.last_undo_group()
		.expect("should have undo group");
	assert_eq!(
		group.view_snapshots.len(),
		2,
		"undo group should have snapshots for both buffers"
	);
	assert!(group.view_snapshots.contains_key(&buffer1_id));
	assert!(group.view_snapshots.contains_key(&buffer2_id));

	editor.undo();

	assert_eq!(
		get_cursor(&editor, buffer1_id),
		7,
		"buffer1 cursor should be restored"
	);
	assert_eq!(
		get_cursor(&editor, buffer2_id),
		15,
		"buffer2 cursor should be restored"
	);
	assert_eq!(
		get_scroll(&editor, buffer1_id),
		(0, 0),
		"buffer1 scroll should be restored"
	);
	assert_eq!(
		get_scroll(&editor, buffer2_id),
		(1, 0),
		"buffer2 scroll should be restored"
	);
}

#[test]
fn redo_restores_view_state() {
	let mut editor = test_editor("hello world");
	set_cursor(&mut editor, 5);
	set_scroll(&mut editor, 0, 0);

	apply_test_edit(&mut editor, " there", 5);

	set_cursor(&mut editor, 10);
	set_scroll(&mut editor, 1, 0);

	editor.undo();

	assert_eq!(get_cursor(&editor, editor.focused_view()), 5);

	editor.redo();

	assert_eq!(
		get_cursor(&editor, editor.focused_view()),
		10,
		"redo should restore cursor from before undo"
	);
	assert_eq!(
		get_scroll(&editor, editor.focused_view()),
		(1, 0),
		"redo should restore scroll from before undo"
	);
}

#[test]
fn redo_stack_clears_on_new_edit() {
	let mut editor = test_editor("hello");

	apply_test_edit(&mut editor, " world", 5);
	assert_eq!(editor.state.core.undo_manager.undo_len(), 1);

	editor.undo();
	assert_eq!(
		editor.state.core.undo_manager.redo_len(),
		1,
		"redo stack should have one entry after undo"
	);

	apply_test_edit(&mut editor, "!", 5);
	assert!(
		!editor.state.core.undo_manager.can_redo(),
		"new edit should clear redo stack"
	);
}

#[test]
fn redo_stack_clears_only_when_group_pushed() {
	let mut editor = test_editor("hello");

	apply_test_edit(&mut editor, " world", 5);
	editor.undo();
	assert_eq!(editor.state.core.undo_manager.redo_len(), 1);

	let buffer_id = editor.focused_view();
	let tx = {
		let buffer = editor.state.core.buffers.focused_buffer();
		let rope = buffer.with_doc(|doc| doc.content().clone());
		Transaction::change(
			rope.slice(..),
			[Change {
				start: 0,
				end: 0,
				replacement: Some("X".into()),
			}],
		)
	};
	editor.apply_edit(
		buffer_id,
		&tx,
		None,
		UndoPolicy::NoUndo,
		EditOrigin::Internal("test"),
	);

	assert_eq!(
		editor.state.core.undo_manager.redo_len(),
		1,
		"NoUndo edit should not clear redo stack"
	);
}

#[test]
fn merge_with_current_group_creates_single_undo_group_for_consecutive_inserts() {
	let mut editor = test_editor("hello");

	let buffer_id = editor.focused_view();
	let tx1 = {
		let buffer = editor.state.core.buffers.focused_buffer();
		let rope = buffer.with_doc(|doc| doc.content().clone());
		Transaction::change(
			rope.slice(..),
			[Change {
				start: 5,
				end: 5,
				replacement: Some("A".into()),
			}],
		)
	};
	editor.apply_edit(
		buffer_id,
		&tx1,
		None,
		UndoPolicy::MergeWithCurrentGroup,
		EditOrigin::Internal("insert"),
	);

	assert_eq!(
		editor.state.core.undo_manager.undo_len(),
		1,
		"first MergeWithCurrentGroup should create group"
	);

	let tx2 = {
		let buffer = editor.state.core.buffers.focused_buffer();
		let rope = buffer.with_doc(|doc| doc.content().clone());
		Transaction::change(
			rope.slice(..),
			[Change {
				start: 6,
				end: 6,
				replacement: Some("B".into()),
			}],
		)
	};
	editor.apply_edit(
		buffer_id,
		&tx2,
		None,
		UndoPolicy::MergeWithCurrentGroup,
		EditOrigin::Internal("insert"),
	);

	assert_eq!(
		editor.state.core.undo_manager.undo_len(),
		1,
		"consecutive MergeWithCurrentGroup should NOT create new group"
	);

	editor.undo();
	let content = editor
		.state
		.core
		.buffers
		.focused_buffer()
		.with_doc(|doc| doc.content().to_string());
	assert_eq!(
		content, "hello",
		"single undo should revert both merged edits"
	);
}

#[test]
fn record_policy_breaks_merge_group() {
	let mut editor = test_editor("hello");
	let buffer_id = editor.focused_view();

	let tx1 = {
		let buffer = editor.state.core.buffers.focused_buffer();
		let rope = buffer.with_doc(|doc| doc.content().clone());
		Transaction::change(
			rope.slice(..),
			[Change {
				start: 5,
				end: 5,
				replacement: Some("A".into()),
			}],
		)
	};
	editor.apply_edit(
		buffer_id,
		&tx1,
		None,
		UndoPolicy::MergeWithCurrentGroup,
		EditOrigin::Internal("insert"),
	);

	let tx2 = {
		let buffer = editor.state.core.buffers.focused_buffer();
		let rope = buffer.with_doc(|doc| doc.content().clone());
		Transaction::change(
			rope.slice(..),
			[Change {
				start: 6,
				end: 6,
				replacement: Some("B".into()),
			}],
		)
	};
	editor.apply_edit(
		buffer_id,
		&tx2,
		None,
		UndoPolicy::Record,
		EditOrigin::Internal("edit"),
	);

	assert_eq!(
		editor.state.core.undo_manager.undo_len(),
		2,
		"Record policy should create new group"
	);
}

#[test]
fn sibling_selection_sync_after_apply() {
	let mut editor = test_editor("abcd");
	let buffer1_id = editor.focused_view();
	let buffer2_id = editor.state.core.buffers.clone_focused_buffer_for_split();

	let original_selection = {
		let buffer = editor
			.state
			.core
			.buffers
			.get_buffer_mut(buffer2_id)
			.expect("split buffer exists");
		let selection = Selection::single(1, 3);
		buffer.set_cursor_and_selection(3, selection.clone());
		selection
	};

	let (tx, new_selection) = {
		let buffer = editor
			.state
			.core
			.buffers
			.get_buffer_mut(buffer1_id)
			.expect("buffer exists");
		buffer.set_cursor_and_selection(0, Selection::single(0, 0));
		buffer.prepare_insert("Z")
	};

	let applied = editor.apply_edit(
		buffer1_id,
		&tx,
		Some(new_selection),
		UndoPolicy::Record,
		EditOrigin::Internal("test"),
	);
	assert!(applied);

	let expected = tx.map_selection(&original_selection);
	let buffer2 = editor
		.state
		.core
		.buffers
		.get_buffer(buffer2_id)
		.expect("split buffer exists");
	assert_eq!(buffer2.selection, expected);
}

proptest! {
	#[test]
	fn undo_redo_roundtrip(ops in prop::collection::vec((0usize..100, "[a-z]{1,3}"), 1..20)) {
		let mut editor = test_editor("");
		let buffer_id = editor.focused_view();
		let steps = ops.len();

		for (pos_seed, text) in ops {
			let len = editor.state.core
				.buffers
				.get_buffer(buffer_id)
				.expect("buffer exists")
				.with_doc(|doc| doc.content().len_chars());
			let pos = if len == 0 { 0 } else { pos_seed % (len + 1) };
			{
				let buffer = editor.state.core
					.buffers
					.get_buffer_mut(buffer_id)
					.expect("buffer exists");
				buffer.set_cursor_and_selection(pos, Selection::single(pos, pos));
			}
			editor.insert_text(&text);
		}

		let (final_text, final_selection, final_cursor) = {
			let buffer = editor.state.core
				.buffers
				.get_buffer(buffer_id)
				.expect("buffer exists");
			(
				buffer.with_doc(|doc| doc.content().to_string()),
				buffer.selection.clone(),
				buffer.cursor,
			)
		};

		for _ in 0..steps {
			editor.undo();
		}
		for _ in 0..steps {
			editor.redo();
		}

		let buffer = editor.state.core
			.buffers
			.get_buffer(buffer_id)
			.expect("buffer exists");
		let redo_text = buffer.with_doc(|doc| doc.content().to_string());
		prop_assert_eq!(redo_text, final_text);
		prop_assert_eq!(&buffer.selection, &final_selection);
		prop_assert_eq!(buffer.cursor, final_cursor);
	}

	#[test]
	fn insert_group_boundaries(ops in prop::collection::vec(any::<bool>(), 1..40)) {
		let mut editor = test_editor("");
		let buffer_id = editor.focused_view();
		let mut expected_groups = 0;
		let mut in_insert_group = false;

		for is_insert in ops {
			let (tx, new_selection) = {
				let buffer = editor.state.core
					.buffers
					.get_buffer_mut(buffer_id)
					.expect("buffer exists");
				let len = buffer.with_doc(|doc| doc.content().len_chars());
				buffer.set_cursor_and_selection(len, Selection::single(len, len));
				buffer.prepare_insert(if is_insert { "i" } else { "x" })
			};

			let undo = if is_insert {
				if !in_insert_group {
					expected_groups += 1;
					in_insert_group = true;
				}
				UndoPolicy::MergeWithCurrentGroup
			} else {
				expected_groups += 1;
				in_insert_group = false;
				UndoPolicy::Record
			};

			let applied = editor.apply_edit(
				buffer_id,
				&tx,
				Some(new_selection),
				undo,
				EditOrigin::Internal("prop"),
			);
			prop_assert!(applied);
		}

		prop_assert_eq!(editor.state.core.undo_manager.undo_len(), expected_groups);
	}
}
