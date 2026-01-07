use ropey::Rope;

use crate::client::OffsetEncoding;
use crate::position::{char_range_to_lsp_range, char_to_lsp_position};
use xeno_base::lsp::{LspDocumentChange, LspPosition, LspRange};
use xeno_base::transaction::Operation;
use xeno_base::Transaction;

/// Computes LSP change events from a transaction against pre-change text.
pub fn compute_lsp_changes(
	rope: &Rope,
	tx: &Transaction,
	encoding: OffsetEncoding,
) -> Vec<LspDocumentChange> {
	let mut changes = Vec::new();
	if tx.changes().is_empty() {
		return changes;
	}

	let mut scratch = rope.clone();
	let mut pos = 0usize;

	for op in tx.operations() {
		match op {
			Operation::Retain(n) => {
				pos += n;
			}
			Operation::Delete(n) => {
				let end = (pos + n).min(scratch.len_chars());
				let Some(range) =
					char_range_to_lsp_range(&scratch, pos, end, encoding)
				else {
					return Vec::new();
				};
				changes.push(LspDocumentChange {
					range: LspRange::new(
						LspPosition::new(range.start.line, range.start.character),
						LspPosition::new(range.end.line, range.end.character),
					),
					new_text: String::new(),
				});
				scratch.remove(pos..end);
			}
			Operation::Insert(ins) => {
				let Some(lsp_pos) = char_to_lsp_position(&scratch, pos, encoding) else {
					return Vec::new();
				};
				changes.push(LspDocumentChange {
					range: LspRange::point(LspPosition::new(lsp_pos.line, lsp_pos.character)),
					new_text: ins.text.clone(),
				});
				scratch.insert(pos, &ins.text);
				pos += ins.char_len;
			}
		}
	}

	changes
}

#[cfg(test)]
mod tests {
	use super::*;
	use xeno_base::Selection;
	use xeno_base::transaction::Change;

	#[test]
	fn test_insert_computes_correct_range() {
		let rope = Rope::from("hello\nworld\n");
		let sel = Selection::single(6, 6);
		let tx = Transaction::insert(rope.slice(..), &sel, "beautiful ".to_string());

		let changes = compute_lsp_changes(&rope, &tx, OffsetEncoding::Utf16);

		assert_eq!(changes.len(), 1);
		assert_eq!(
			changes[0].range,
			LspRange::point(LspPosition::new(1, 0))
		);
		assert_eq!(changes[0].new_text, "beautiful ");
	}

	#[test]
	fn test_delete_line_computes_correct_range() {
		let rope = Rope::from("line1\nline2\nline3\n");
		let sel = Selection::single(6, 12);
		let tx = Transaction::delete(rope.slice(..), &sel);

		let changes = compute_lsp_changes(&rope, &tx, OffsetEncoding::Utf16);

		assert_eq!(changes.len(), 1);
		assert_eq!(
			changes[0].range,
			LspRange::new(LspPosition::new(1, 0), LspPosition::new(2, 0))
		);
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

		assert_eq!(changes.len(), 2);
		assert_eq!(
			changes[0].range,
			LspRange::point(LspPosition::new(0, 0))
		);
		assert_eq!(changes[0].new_text, "\n");
		assert_eq!(
			changes[1].range,
			LspRange::point(LspPosition::new(2, 0))
		);
		assert_eq!(changes[1].new_text, "X");
	}
}
