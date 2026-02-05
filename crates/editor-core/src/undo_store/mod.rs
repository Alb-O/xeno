//! Undo storage backends for document history.
//!
//! Standardizes on grouped transaction-based undo/redo. Stores edit deltas
//! rather than full snapshots for memory efficiency.
//!
//! # Architecture
//!
//! Document undo is separate from view state. The undo store only manages
//! document content; cursor, selection, and scroll restoration happens in the
//! application layer.

#[cfg(test)]
mod tests;

use xeno_primitives::{Rope, Transaction};

/// Maximum undo history size in steps.
pub const MAX_UNDO: usize = 100;

/// Maximum undo memory usage in bytes (10MB).
pub const MAX_UNDO_BYTES: usize = 10 * 1024 * 1024;

/// A single step in the undo/redo history.
///
/// Bundles multiple transactions that occurred within a single user-perceived
/// operation (e.g. an insert-mode typing run).
#[derive(Debug, Clone)]
pub struct UndoStep {
	/// Transactions applied during the forward operation (for redo).
	pub redo: Vec<Transaction>,
	/// Inverse transactions (for undo), applied in reverse order.
	pub undo: Vec<Transaction>,
	/// Approximate memory usage of this step in bytes.
	pub bytes: usize,
}

impl UndoStep {
	/// Creates a new undo step from a single transaction pair.
	pub fn new(redo_tx: Transaction, undo_tx: Transaction) -> Self {
		let bytes = approx_transaction_bytes(&redo_tx) + approx_transaction_bytes(&undo_tx);
		Self {
			redo: vec![redo_tx],
			undo: vec![undo_tx],
			bytes,
		}
	}

	/// Appends a transaction pair to this step, updating its memory weight.
	pub fn append(&mut self, redo_tx: Transaction, undo_tx: Transaction) {
		self.bytes += approx_transaction_bytes(&redo_tx) + approx_transaction_bytes(&undo_tx);
		self.redo.push(redo_tx);
		self.undo.push(undo_tx);
	}
}

/// Estimates the memory usage of a transaction in bytes.
///
/// Only considers the size of inserted text strings and a small overhead.
fn approx_transaction_bytes(tx: &Transaction) -> usize {
	tx.operations()
		.iter()
		.map(|op| match op {
			xeno_primitives::transaction::Operation::Insert(ins) => ins.text.len(),
			_ => 0,
		})
		.sum::<usize>()
		+ 32
}

/// Transaction-based grouped undo store.
///
/// Stores sequences of forward/reverse transactions instead of full snapshots.
/// More memory-efficient for large documents and correct for grouped edits.
#[derive(Default, Debug)]
pub struct TxnUndoStore {
	undo_stack: Vec<UndoStep>,
	redo_stack: Vec<UndoStep>,
	undo_bytes: usize,
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

	/// Returns the number of steps in the undo stack.
	pub fn undo_len(&self) -> usize {
		self.undo_stack.len()
	}

	/// Returns the number of steps in the redo stack.
	pub fn redo_len(&self) -> usize {
		self.redo_stack.len()
	}

	/// Clears the redo stack.
	///
	/// MUST be called on every new edit to maintain history integrity.
	pub fn clear_redo(&mut self) {
		self.redo_stack.clear();
	}

	/// Records a transaction for undo.
	///
	/// If `merge` is true and a group is already active, appends to it.
	/// Otherwise starts a new undo step. Enforces [`MAX_UNDO`] and
	/// [`MAX_UNDO_BYTES`] limits by evicting the oldest steps.
	pub fn record_transaction(&mut self, redo_tx: Transaction, undo_tx: Transaction, merge: bool) {
		if merge && let Some(step) = self.undo_stack.last_mut() {
			let old_bytes = step.bytes;
			step.append(redo_tx, undo_tx);
			self.undo_bytes += step.bytes - old_bytes;
		} else {
			let step = UndoStep::new(redo_tx, undo_tx);
			self.undo_bytes += step.bytes;
			self.undo_stack.push(step);
		}

		self.enforce_limits();
		self.clear_redo();
	}

	/// Evicts oldest steps until limits are met.
	fn enforce_limits(&mut self) {
		while (self.undo_stack.len() > MAX_UNDO || self.undo_bytes > MAX_UNDO_BYTES)
			&& !self.undo_stack.is_empty()
		{
			let oldest = self.undo_stack.remove(0);
			self.undo_bytes = self.undo_bytes.saturating_sub(oldest.bytes);
		}
	}

	/// Undoes the last change by applying the sequence of inverse transactions.
	///
	/// Returns the applied transactions in the order they were executed
	/// (reverse of the original edit order).
	pub fn undo(&mut self, content: &mut Rope) -> Option<Vec<Transaction>> {
		let step = self.undo_stack.pop()?;
		self.undo_bytes = self.undo_bytes.saturating_sub(step.bytes);

		let mut applied = Vec::with_capacity(step.undo.len());

		for tx in step.undo.iter().rev() {
			tx.apply(content);
			applied.push(tx.clone());
		}

		self.redo_stack.push(step);
		Some(applied)
	}

	/// Redoes the last undone change by applying the sequence of forward transactions.
	///
	/// Returns the applied transactions in forward order.
	pub fn redo(&mut self, content: &mut Rope) -> Option<Vec<Transaction>> {
		let step = self.redo_stack.pop()?;

		for tx in &step.redo {
			tx.apply(content);
		}

		let redo_txs = step.redo.clone();
		self.undo_bytes += step.bytes;
		self.undo_stack.push(step);
		self.enforce_limits();
		Some(redo_txs)
	}
}

/// Unified undo backend.
///
/// Standardized on grouped transaction sequences.
#[derive(Default, Debug)]
pub struct UndoBackend {
	store: TxnUndoStore,
}

impl UndoBackend {
	/// Creates a new empty backend.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns whether undo is available.
	pub fn can_undo(&self) -> bool {
		self.store.can_undo()
	}

	/// Returns whether redo is available.
	pub fn can_redo(&self) -> bool {
		self.store.can_redo()
	}

	/// Returns the number of steps in the undo stack.
	pub fn undo_len(&self) -> usize {
		self.store.undo_len()
	}

	/// Returns the number of steps in the redo stack.
	pub fn redo_len(&self) -> usize {
		self.store.redo_len()
	}

	/// Clears the redo stack.
	pub fn clear_redo(&mut self) {
		self.store.clear_redo();
	}

	/// Records a commit for undo.
	///
	/// If `merge` is true, appends to the current undo group.
	pub fn record_commit(&mut self, tx: &Transaction, before: &Rope, merge: bool) {
		let undo_tx = tx.invert(before);
		self.store.record_transaction(tx.clone(), undo_tx, merge);
	}

	/// Performs undo, updating the document content and version.
	///
	/// Restores document state from the undo stack and increments the version.
	///
	/// Returns the applied inverse transactions if undo was performed.
	pub fn undo(&mut self, content: &mut Rope, version: &mut u64) -> Option<Vec<Transaction>> {
		let applied = self.store.undo(content)?;
		*version = version.wrapping_add(1);
		Some(applied)
	}

	/// Performs redo, updating the document content and version.
	///
	/// Restores document state from the redo stack and increments the version.
	///
	/// Returns the applied forward transactions if redo was performed.
	pub fn redo(&mut self, content: &mut Rope, version: &mut u64) -> Option<Vec<Transaction>> {
		let applied = self.store.redo(content)?;
		*version = version.wrapping_add(1);
		Some(applied)
	}
}
