//! Transaction-to-LSP change translation.
//!
//! Converts editor transactions into incremental LSP document changes using
//! the requested offset encoding, with a safe fallback signal when a precise
//! incremental mapping cannot be produced.

use ropey::Rope;
use xeno_primitives::Transaction;
use xeno_primitives::lsp::{LspDocumentChange, LspPosition, LspRange};
use xeno_primitives::transaction::Operation;

use crate::client::OffsetEncoding;
use crate::position::{char_range_to_lsp_range, char_to_lsp_position};

/// Result of computing incremental LSP changes from a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncrementalResult {
	/// Incremental changes were computed successfully.
	Incremental(Vec<LspDocumentChange>),
	/// Failed to compute incremental changes, fallback to full sync.
	FallbackToFull,
}

/// Computes LSP change events from a transaction against pre-change text.
pub fn compute_lsp_changes(rope: &Rope, tx: &Transaction, encoding: OffsetEncoding) -> IncrementalResult {
	let mut changes = Vec::new();
	if tx.changes().is_empty() {
		return IncrementalResult::Incremental(changes);
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
				let Some(range) = char_range_to_lsp_range(&scratch, pos, end, encoding) else {
					return IncrementalResult::FallbackToFull;
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
					return IncrementalResult::FallbackToFull;
				};
				changes.push(LspDocumentChange {
					range: LspRange::point(LspPosition::new(lsp_pos.line, lsp_pos.character)),
					new_text: ins.text().to_owned(),
				});
				scratch.insert(pos, ins.text());
				pos += ins.char_len();
			}
		}
	}

	IncrementalResult::Incremental(changes)
}

#[cfg(test)]
mod tests;
