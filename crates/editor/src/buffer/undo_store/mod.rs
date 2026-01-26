//! Undo storage backends for document history.
//!
//! This module provides pluggable undo backends through the [`UndoBackend`] enum:
//!
//! - [`SnapshotUndoStore`]: Stores full rope snapshots (current behavior)
//! - [`TxnUndoStore`]: Stores transaction pairs for efficient undo/redo
//!
//! # Architecture
//!
//! Document undo is separate from editor-level view state. The undo store only
//! manages document content - cursor, selection, and scroll restoration happens
//! at the editor level via [`EditorUndoGroup`].
//!
//! [`EditorUndoGroup`]: crate::types::EditorUndoGroup

#[cfg(test)]
mod tests;

use xeno_primitives::{Rope, Transaction};
use xeno_runtime_language::LanguageLoader;

/// Maximum undo history size.
pub const MAX_UNDO: usize = 100;

/// Snapshot of document state for undo operations.
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
	/// Document text content.
	pub rope: Rope,
	/// Document version at snapshot time.
	pub version: u64,
}

/// Snapshot-based undo step (current behavior).
///
/// Stores a complete copy of the document. Memory-efficient due to rope's
/// structural sharing, but still holds references to old rope nodes.
#[derive(Clone)]
pub struct SnapshotUndoStep {
	/// Document content at this point in history.
	pub rope: Rope,
	/// Document version at this point in history.
	pub version: u64,
	/// Transaction to apply for undo (reverses the original edit).
	pub undo_tx: Transaction,
	/// Transaction to apply for redo (re-applies the original edit).
	pub redo_tx: Transaction,
}

/// Transaction-based undo step (new behavior).
///
/// Stores the forward and reverse transactions. More efficient for large
/// documents with small edits since it only stores the delta.
#[derive(Debug, Clone)]
pub struct TxnUndoStep {
	/// Transaction to apply for undo (reverses the original edit).
	pub undo_tx: Transaction,
	/// Transaction to apply for redo (re-applies the original edit).
	pub redo_tx: Transaction,
}

/// Snapshot-based undo store.
///
/// Stores full rope snapshots for each undo step. Simple and reliable due to
/// rope's structural sharing, but holds references to old rope nodes which
/// can increase memory usage under heavy editing.
///
/// This is the default backend for backward compatibility.
#[derive(Default)]
pub struct SnapshotUndoStore {
	undo_stack: Vec<SnapshotUndoStep>,
	redo_stack: Vec<SnapshotUndoStep>,
}

impl SnapshotUndoStore {
	/// Creates a new empty snapshot store.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns whether undo is available.
	pub fn can_undo(&self) -> bool {
		!self.undo_stack.is_empty()
	}

	/// Returns whether redo is available.
	pub fn can_redo(&self) -> bool {
		!self.redo_stack.is_empty()
	}

	/// Returns the undo stack length.
	pub fn undo_len(&self) -> usize {
		self.undo_stack.len()
	}

	/// Returns the redo stack length.
	pub fn redo_len(&self) -> usize {
		self.redo_stack.len()
	}

	/// Clears the redo stack (called after a new edit).
	pub fn clear_redo(&mut self) {
		self.redo_stack.clear();
	}

	/// Records a snapshot before an edit.
	///
	/// Call this before applying the transaction to capture the pre-edit state.
	/// Automatically enforces [`MAX_UNDO`] limit by removing the oldest entry.
	pub fn record_snapshot(&mut self, snapshot: DocumentSnapshot, tx: &Transaction) {
		let undo_tx = tx.invert(&snapshot.rope);
		self.undo_stack.push(SnapshotUndoStep {
			rope: snapshot.rope,
			version: snapshot.version,
			undo_tx: undo_tx.clone(),
			redo_tx: tx.clone(),
		});
		self.redo_stack.clear();

		if self.undo_stack.len() > MAX_UNDO {
			self.undo_stack.remove(0);
		}
	}

	/// Undoes the last change.
	///
	/// Pops the most recent snapshot from the undo stack and saves the current
	/// state to the redo stack. Returns the restored document state, or `None`
	/// if the undo stack is empty.
	pub fn undo(&mut self, current: DocumentSnapshot) -> Option<(DocumentSnapshot, Transaction)> {
		let step = self.undo_stack.pop()?;
		let SnapshotUndoStep {
			rope,
			version,
			undo_tx,
			redo_tx,
		} = step;

		self.redo_stack.push(SnapshotUndoStep {
			rope: current.rope,
			version: current.version,
			undo_tx: undo_tx.clone(),
			redo_tx: redo_tx.clone(),
		});

		Some((DocumentSnapshot { rope, version }, undo_tx))
	}

	/// Redoes the last undone change.
	///
	/// Pops the most recent snapshot from the redo stack and saves the current
	/// state to the undo stack. Returns the restored document state, or `None`
	/// if the redo stack is empty.
	pub fn redo(&mut self, current: DocumentSnapshot) -> Option<(DocumentSnapshot, Transaction)> {
		let step = self.redo_stack.pop()?;
		let SnapshotUndoStep {
			rope,
			version,
			undo_tx,
			redo_tx,
		} = step;

		self.undo_stack.push(SnapshotUndoStep {
			rope: current.rope,
			version: current.version,
			undo_tx: undo_tx.clone(),
			redo_tx: redo_tx.clone(),
		});

		Some((DocumentSnapshot { rope, version }, redo_tx))
	}
}

/// Transaction-based undo store.
///
/// Stores transaction pairs (undo, redo) instead of full rope snapshots.
/// More memory-efficient for large documents with small edits since only
/// the edit delta is stored, not a full document copy.
///
/// Each undo step contains the inverse transaction computed via
/// [`Transaction::invert`](xeno_primitives::Transaction::invert).
#[derive(Default)]
pub struct TxnUndoStore {
	undo_stack: Vec<TxnUndoStep>,
	redo_stack: Vec<TxnUndoStep>,
}

impl TxnUndoStore {
	/// Creates a new empty transaction store.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns whether undo is available.
	pub fn can_undo(&self) -> bool {
		!self.undo_stack.is_empty()
	}

	/// Returns whether redo is available.
	pub fn can_redo(&self) -> bool {
		!self.redo_stack.is_empty()
	}

	/// Returns the undo stack length.
	pub fn undo_len(&self) -> usize {
		self.undo_stack.len()
	}

	/// Returns the redo stack length.
	pub fn redo_len(&self) -> usize {
		self.redo_stack.len()
	}

	/// Clears the redo stack (called after a new edit).
	pub fn clear_redo(&mut self) {
		self.redo_stack.clear();
	}

	/// Records a transaction for undo.
	///
	/// Computes the inverse transaction by calling `invert()` on the forward
	/// transaction against the pre-edit document state. Both transactions are
	/// stored so undo applies the inverse and redo re-applies the original.
	///
	/// Automatically enforces [`MAX_UNDO`] limit by removing the oldest entry.
	pub fn record_transaction(&mut self, redo_tx: Transaction, before: &DocumentSnapshot) {
		let undo_tx = redo_tx.invert(&before.rope);

		self.undo_stack.push(TxnUndoStep { undo_tx, redo_tx });
		self.redo_stack.clear();

		if self.undo_stack.len() > MAX_UNDO {
			self.undo_stack.remove(0);
		}
	}

	/// Undoes the last change by applying the inverse transaction.
	///
	/// Returns the transaction to apply, or None if nothing to undo.
	/// The caller must apply the returned transaction to the document.
	pub fn undo(&mut self) -> Option<&Transaction> {
		let step = self.undo_stack.last()?;
		Some(&step.undo_tx)
	}

	/// Commits the undo operation after successfully applying it.
	///
	/// Moves the step from undo stack to redo stack.
	pub fn commit_undo(&mut self) {
		if let Some(step) = self.undo_stack.pop() {
			self.redo_stack.push(step);
		}
	}

	/// Redoes the last undone change by applying the forward transaction.
	///
	/// Returns the transaction to apply, or None if nothing to redo.
	/// The caller must apply the returned transaction to the document.
	pub fn redo(&mut self) -> Option<&Transaction> {
		let step = self.redo_stack.last()?;
		Some(&step.redo_tx)
	}

	/// Commits the redo operation after successfully applying it.
	///
	/// Moves the step from redo stack back to undo stack.
	pub fn commit_redo(&mut self) {
		if let Some(step) = self.redo_stack.pop() {
			self.undo_stack.push(step);
		}
	}
}

/// Unified undo backend supporting multiple storage strategies.
///
/// Uses an enum rather than trait objects to avoid vtable overhead.
/// [`Document`](super::Document) owns this and delegates undo operations to it.
///
/// # Variants
///
/// - [`Snapshot`](Self::Snapshot): Stores full rope copies (default, simpler)
/// - [`Transaction`](Self::Transaction): Stores edit deltas (more memory-efficient)
pub enum UndoBackend {
	/// Snapshot-based undo storing full rope copies.
	Snapshot(SnapshotUndoStore),
	/// Transaction-based undo storing edit deltas.
	Transaction(TxnUndoStore),
}

impl Default for UndoBackend {
	/// Returns snapshot-based backend for backward compatibility.
	fn default() -> Self {
		Self::Snapshot(SnapshotUndoStore::new())
	}
}

impl UndoBackend {
	/// Creates a new snapshot-based backend.
	pub fn snapshot() -> Self {
		Self::Snapshot(SnapshotUndoStore::new())
	}

	/// Creates a new transaction-based backend.
	pub fn transaction() -> Self {
		Self::Transaction(TxnUndoStore::new())
	}

	/// Returns whether undo is available.
	pub fn can_undo(&self) -> bool {
		match self {
			Self::Snapshot(s) => s.can_undo(),
			Self::Transaction(t) => t.can_undo(),
		}
	}

	/// Returns whether redo is available.
	pub fn can_redo(&self) -> bool {
		match self {
			Self::Snapshot(s) => s.can_redo(),
			Self::Transaction(t) => t.can_redo(),
		}
	}

	/// Returns the undo stack length.
	pub fn undo_len(&self) -> usize {
		match self {
			Self::Snapshot(s) => s.undo_len(),
			Self::Transaction(t) => t.undo_len(),
		}
	}

	/// Returns the redo stack length.
	pub fn redo_len(&self) -> usize {
		match self {
			Self::Snapshot(s) => s.redo_len(),
			Self::Transaction(t) => t.redo_len(),
		}
	}

	/// Clears the redo stack.
	pub fn clear_redo(&mut self) {
		match self {
			Self::Snapshot(s) => s.clear_redo(),
			Self::Transaction(t) => t.clear_redo(),
		}
	}

	/// Records a commit for undo.
	///
	/// For snapshot backend: records a snapshot of the pre-edit state.
	/// For transaction backend: records the transaction with its inverse.
	pub fn record_commit(&mut self, tx: &Transaction, before: &DocumentSnapshot) {
		match self {
			Self::Snapshot(s) => {
				s.record_snapshot(before.clone(), tx);
			}
			Self::Transaction(t) => {
				t.record_transaction(tx.clone(), before);
			}
		}
	}

	/// Performs undo, updating the document content and version.
	///
	/// Restores document state from the undo stack and increments the version.
	/// The `reparse` closure is called after content restoration for syntax updates.
	///
	/// Returns the applied transaction if undo was performed.
	pub fn undo(
		&mut self,
		content: &mut Rope,
		version: &mut u64,
		language_loader: &LanguageLoader,
		reparse: impl FnOnce(&mut Rope, &LanguageLoader),
	) -> Option<Transaction> {
		match self {
			Self::Snapshot(s) => {
				let current = DocumentSnapshot {
					rope: content.clone(),
					version: *version,
				};
				if let Some((restored, tx)) = s.undo(current) {
					*content = restored.rope;
					*version = version.wrapping_add(1);
					reparse(content, language_loader);
					Some(tx)
				} else {
					None
				}
			}
			Self::Transaction(t) => {
				if let Some(tx) = t.undo().cloned() {
					tx.apply(content);
					*version = version.wrapping_add(1);
					t.commit_undo();
					reparse(content, language_loader);
					Some(tx)
				} else {
					None
				}
			}
		}
	}

	/// Performs redo, updating the document content and version.
	///
	/// Restores document state from the redo stack and increments the version.
	/// The `reparse` closure is called after content restoration for syntax updates.
	///
	/// Returns the applied transaction if redo was performed.
	pub fn redo(
		&mut self,
		content: &mut Rope,
		version: &mut u64,
		language_loader: &LanguageLoader,
		reparse: impl FnOnce(&mut Rope, &LanguageLoader),
	) -> Option<Transaction> {
		match self {
			Self::Snapshot(s) => {
				let current = DocumentSnapshot {
					rope: content.clone(),
					version: *version,
				};
				if let Some((restored, tx)) = s.redo(current) {
					*content = restored.rope;
					*version = version.wrapping_add(1);
					reparse(content, language_loader);
					Some(tx)
				} else {
					None
				}
			}
			Self::Transaction(t) => {
				if let Some(tx) = t.redo().cloned() {
					tx.apply(content);
					*version = version.wrapping_add(1);
					t.commit_redo();
					reparse(content, language_loader);
					Some(tx)
				} else {
					None
				}
			}
		}
	}
}
