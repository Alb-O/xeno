use xeno_runtime_language::LanguageLoader;

use crate::buffer::{ApplyPolicy, Buffer, ViewId};

#[cfg(feature = "lsp")]
mod lsp_batching {
	use xeno_lsp::{IncrementalResult, OffsetEncoding, compute_lsp_changes};
	use xeno_primitives::lsp::{LspPosition, LspRange};
	use xeno_primitives::{Selection, SyntaxPolicy};
	use xeno_runtime_language::LanguageLoader;

	use crate::buffer::{ApplyPolicy, Buffer, ViewId};

	/// Policy shorthand for LSP-synced edits without undo.
	const LSP_POLICY: ApplyPolicy = ApplyPolicy::BARE.with_syntax(SyntaxPolicy::IncrementalOrDirty);

	fn make_buffer(content: &str) -> Buffer {
		let buffer = Buffer::scratch(ViewId::SCRATCH);
		if !content.is_empty() {
			let rope = ropey::Rope::from(content);
			buffer.with_doc_mut(|doc| doc.reset_content(rope));
		}
		buffer
	}

	#[test]
	fn single_insert_returns_one_change() {
		let mut buffer = make_buffer("hello");
		// Selection on 'o' (last cell)
		buffer.set_selection(Selection::point(4));

		// Insert after 'o' (simulate 'a')
		let (tx, _sel) = buffer.prepare_paste_after(" world").unwrap();
		let loader = xeno_runtime_language::LanguageLoader::new();
		let result = buffer.apply_with_lsp(&tx, LSP_POLICY, &loader, OffsetEncoding::Utf16);

		assert!(result.commit.applied);
		let changes = result.lsp_changes.expect("should have changes");
		assert_eq!(changes.len(), 1);
		assert_eq!(changes[0].range, LspRange::point(LspPosition::new(0, 5)));
		assert_eq!(changes[0].new_text, " world");
	}

	#[test]
	fn multiple_transactions_return_changes() {
		let mut buffer = make_buffer("line1\nline2\n");
		let loader = xeno_runtime_language::LanguageLoader::new();

		// First transaction: insert at start of line 1
		buffer.set_selection(Selection::single(0, 0));
		let (tx1, sel1) = buffer.prepare_insert("A");
		let result1 = buffer.apply_with_lsp(&tx1, LSP_POLICY, &loader, OffsetEncoding::Utf16);
		buffer.finalize_selection(sel1);
		let changes1 = result1.lsp_changes.expect("should have changes");

		// Second transaction: insert at start of line 2
		// After first insert, "Aline1\nline2\n", line 2 starts at char 7
		buffer.set_selection(Selection::single(7, 7));
		let (tx2, sel2) = buffer.prepare_insert("B");
		let result2 = buffer.apply_with_lsp(&tx2, LSP_POLICY, &loader, OffsetEncoding::Utf16);
		buffer.finalize_selection(sel2);
		let changes2 = result2.lsp_changes.expect("should have changes");

		// First change: insert "A" at (0, 0) in original doc
		assert_eq!(changes1.len(), 1);
		assert_eq!(changes1[0].range, LspRange::point(LspPosition::new(0, 0)));
		assert_eq!(changes1[0].new_text, "A");

		// Second change: insert "B" at (1, 0) in doc after first change
		// The position is computed against the state AFTER first transaction
		assert_eq!(changes2.len(), 1);
		assert_eq!(changes2[0].range, LspRange::point(LspPosition::new(1, 0)));
		assert_eq!(changes2[0].new_text, "B");
	}

	#[test]
	fn multi_cursor_single_transaction_returns_ordered_changes() {
		let mut buffer = make_buffer("aaa\nbbb\nccc\n");
		let loader = xeno_runtime_language::LanguageLoader::new();

		// Multi-cursor: start of each line
		buffer.set_selection(Selection::from_vec(
			vec![
				xeno_primitives::Range::point(0),
				xeno_primitives::Range::point(4),
				xeno_primitives::Range::point(8),
			],
			0,
		));

		let (tx, _sel) = buffer.prepare_insert("X");
		let result = buffer.apply_with_lsp(&tx, LSP_POLICY, &loader, OffsetEncoding::Utf16);

		let changes = result.lsp_changes.expect("should have changes");
		assert_eq!(changes.len(), 3);

		// Changes are ordered by position in pre-change document,
		// but positions are computed as transaction is applied
		assert_eq!(changes[0].range, LspRange::point(LspPosition::new(0, 0)));
		assert_eq!(changes[0].new_text, "X");

		// After first insert: "Xaaa\n..." - second cursor was at char 4,
		// but in scratch rope it's at original position since we track shifts
		assert_eq!(changes[1].range, LspRange::point(LspPosition::new(1, 0)));
		assert_eq!(changes[1].new_text, "X");

		assert_eq!(changes[2].range, LspRange::point(LspPosition::new(2, 0)));
		assert_eq!(changes[2].new_text, "X");
	}

	#[test]
	fn incremental_changes_match_reference() {
		let mut buffer = make_buffer("hello");
		buffer.set_selection(Selection::single(5, 5));

		let (tx, new_sel) = buffer.prepare_insert("!");
		let expected =
			buffer.with_doc(|doc| compute_lsp_changes(doc.content(), &tx, OffsetEncoding::Utf16));

		let loader = LanguageLoader::new();
		let result = buffer.apply_with_lsp(
			&tx,
			ApplyPolicy::BARE.with_syntax(SyntaxPolicy::IncrementalOrDirty),
			&loader,
			OffsetEncoding::Utf16,
		);
		assert!(result.commit.applied);
		buffer.finalize_selection(new_sel);

		match expected {
			IncrementalResult::Incremental(changes) => {
				let actual = result.lsp_changes.expect("should have changes");
				assert_eq!(actual, changes);
			}
			IncrementalResult::FallbackToFull => {
				assert!(result.lsp_changes.is_none());
			}
		}
	}

	#[test]
	fn readonly_blocks_lsp_changes() {
		let mut buffer = make_buffer("test");
		let loader = xeno_runtime_language::LanguageLoader::new();

		buffer.set_readonly(true);
		buffer.set_selection(Selection::single(4, 4));
		let (tx, _sel) = buffer.prepare_insert("!");
		let result = buffer.apply_with_lsp(&tx, LSP_POLICY, &loader, OffsetEncoding::Utf16);

		assert!(!result.commit.applied);
		// Blocked commits return empty changes
		let changes = result.lsp_changes.expect("should have empty changes");
		assert!(changes.is_empty());
	}
}

#[test]
fn readonly_flag_roundtrip() {
	let buffer = Buffer::scratch(ViewId::SCRATCH);
	assert!(!buffer.is_readonly());
	buffer.set_readonly(true);
	assert!(buffer.is_readonly());
}

#[test]
fn readonly_blocks_apply_transaction() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	let (tx, _selection) = buffer.prepare_insert("hi");
	buffer.set_readonly(true);
	let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
	assert!(!result.applied);
	assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "");
}

#[test]
fn readonly_override_blocks_transaction() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	assert!(!buffer.with_doc(|doc| doc.is_readonly()));
	buffer.set_readonly_override(Some(true));
	assert!(buffer.is_readonly());

	let (tx, _selection) = buffer.prepare_insert("hi");
	let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
	assert!(!result.applied);
	assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "");
}

#[test]
fn readonly_override_allows_write_on_readonly_doc() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	// Document is readonly, but buffer override makes it writable
	buffer.set_readonly(true);
	assert!(buffer.is_readonly());

	buffer.set_readonly_override(Some(false));
	assert!(!buffer.is_readonly());

	let (tx, _selection) = buffer.prepare_insert("hi");
	let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
	assert!(result.applied);
	assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "hi");
}

#[test]
fn readonly_override_none_defers_to_document() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	buffer.set_readonly_override(None);
	assert!(!buffer.is_readonly()); // Document is writable

	buffer.set_readonly(true);
	assert!(buffer.is_readonly()); // Now document is readonly, override defers
}

#[test]
fn split_does_not_inherit_readonly_override() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	buffer.set_readonly_override(Some(true));
	assert!(buffer.is_readonly());

	let split = buffer.clone_for_split(ViewId(1));
	// Split should defer to document (writable), not inherit override
	assert!(!split.is_readonly());
}
