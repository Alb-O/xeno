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

use std::collections::VecDeque;

use xeno_primitives::{Rope, Transaction, UndoPolicy, ViewId};

/// Maximum undo history size in steps.
pub const MAX_UNDO: usize = 100;

/// Maximum undo memory usage in bytes (10MB).
pub const MAX_UNDO_BYTES: usize = 10 * 1024 * 1024;

/// A single logical step in the undo/redo history.
///
/// Bundles multiple transactions that occurred within a single user-perceived
/// operation (e.g. an insert-mode typing run). This ensures that undoing a
/// typing run reverts all characters at once.
#[derive(Debug, Clone)]
pub struct UndoStep {
	/// Forward transactions applied during the operation.
	pub redo: Vec<Transaction>,
	/// Inverse transactions, applied in reverse order to undo the operation.
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
/// Only considers the size of inserted text strings and a small struct overhead.
fn approx_transaction_bytes(tx: &Transaction) -> usize {
	tx.operations()
		.iter()
		.map(|op| match op {
			xeno_primitives::transaction::Operation::Insert(ins) => ins.byte_len(),
			_ => 0,
		})
		.sum::<usize>()
		+ 32
}

/// Transaction-based grouped undo store.
///
/// Stores sequences of forward/reverse transactions instead of full snapshots.
/// More memory-efficient for large documents and correct for grouped edits.
///
/// # Invariants
///
/// - `undo_bytes` and `redo_bytes` MUST accurately reflect the sum of `bytes` in
///   their respective stacks.
/// - The total memory usage (`undo_bytes + redo_bytes`) MUST NOT exceed [`MAX_UNDO_BYTES`].
#[derive(Default, Debug)]
pub struct TxnUndoStore {
	undo_stack: VecDeque<UndoStep>,
	redo_stack: VecDeque<UndoStep>,
	undo_bytes: usize,
	redo_bytes: usize,
	/// View that owns the current active undo group. Used for cross-view
	/// grouping resolution.
	pub(crate) active_group_owner: Option<ViewId>,
	/// Cached total number of transactions in the undo stack.
	undo_tx_count: u64,
	/// Cached total number of transactions in the redo stack.
	redo_tx_count: u64,
	/// Total number of content-changing transactions applied since history reset.
	head_pos: u64,
	/// The transaction count that matches the on-disk content.
	clean_pos: Option<u64>,
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

	/// Clears the redo stack and resets its memory counter.
	///
	/// MUST be called on every new edit to maintain history integrity.
	pub fn clear_redo(&mut self) {
		self.redo_stack.clear();
		self.redo_bytes = 0;
		self.redo_tx_count = 0;
	}

	/// Records a transaction for undo.
	///
	/// If `merge` is true and a group is already active, appends to it.
	/// Otherwise starts a new undo step.
	///
	/// # Returns
	///
	/// `true` if a new undo step was created, `false` if it was merged.
	///
	/// # Side Effects
	///
	/// - Clears the redo stack.
	/// - Evicts oldest steps if [`MAX_UNDO`] or [`MAX_UNDO_BYTES`] limits are met.
	/// - Invalidates `clean_pos` if history branches away from it.
	pub fn record_transaction(
		&mut self,
		redo_tx: Transaction,
		undo_tx: Transaction,
		merge: bool,
	) -> bool {
		// Invalidate clean_pos if a new edit branches history.
		// We check against old head_pos before incrementing.
		if !self.redo_stack.is_empty()
			&& let Some(cp) = self.clean_pos
			&& cp > self.head_pos
		{
			self.clean_pos = None;
		}

		let new_step_created = if merge && let Some(step) = self.undo_stack.back_mut() {
			let old_bytes = step.bytes;
			step.append(redo_tx, undo_tx);
			self.undo_bytes += step.bytes - old_bytes;
			false
		} else {
			let step = UndoStep::new(redo_tx, undo_tx);
			self.undo_bytes += step.bytes;
			self.undo_stack.push_back(step);
			true
		};

		self.head_pos += 1;
		self.undo_tx_count += 1;

		self.clear_redo();
		self.enforce_limits();

		#[cfg(debug_assertions)]
		self.assert_invariants();

		new_step_created
	}

	/// Evicts oldest steps until memory and depth limits are met.
	///
	/// Prioritizes keeping undo history over redo history when memory is tight.
	fn enforce_limits(&mut self) {
		self.enforce_depth();
		self.enforce_bytes();
		self.invalidate_clean_pos_after_eviction();
	}

	fn enforce_depth(&mut self) {
		while self.undo_stack.len() > MAX_UNDO {
			if let Some(oldest) = self.undo_stack.pop_front() {
				self.undo_bytes = self.undo_bytes.saturating_sub(oldest.bytes);
				self.undo_tx_count -= oldest.undo.len() as u64;
			}
		}
		while self.redo_stack.len() > MAX_UNDO {
			if let Some(oldest) = self.redo_stack.pop_front() {
				self.redo_bytes = self.redo_bytes.saturating_sub(oldest.bytes);
				self.redo_tx_count -= oldest.redo.len() as u64;
			}
		}
	}

	fn enforce_bytes(&mut self) {
		// Enforce total memory cap (undo + redo)
		// Prioritize keeping UNDO over REDO by evicting REDO first.
		while self.undo_bytes + self.redo_bytes > MAX_UNDO_BYTES {
			if let Some(oldest) = self.redo_stack.pop_front() {
				self.redo_bytes = self.redo_bytes.saturating_sub(oldest.bytes);
				self.redo_tx_count -= oldest.redo.len() as u64;
			} else if let Some(oldest) = self.undo_stack.pop_front() {
				self.undo_bytes = self.undo_bytes.saturating_sub(oldest.bytes);
				self.undo_tx_count -= oldest.undo.len() as u64;
			} else {
				break;
			}
		}
	}

	fn invalidate_clean_pos_after_eviction(&mut self) {
		// Invalidate clean_pos if it points to evicted history.
		if let Some(cp) = self.clean_pos {
			let min_undo_pos = self.head_pos.saturating_sub(self.undo_tx_count);
			let max_redo_pos = self.head_pos + self.redo_tx_count;

			if cp < min_undo_pos || cp > max_redo_pos {
				self.clean_pos = None;
			}
		}
	}

	/// Undoes the last change by applying the sequence of inverse transactions.
	///
	/// Returns the applied transactions in the order they were executed
	/// (reverse of the original edit order).
	pub fn undo(&mut self, content: &mut Rope) -> Option<Vec<Transaction>> {
		self.active_group_owner = None;
		let step = self.undo_stack.pop_back()?;
		let tx_len = step.undo.len() as u64;

		debug_assert!(self.head_pos >= tx_len, "head_pos underflow during undo");

		self.undo_bytes = self.undo_bytes.saturating_sub(step.bytes);
		self.undo_tx_count -= tx_len;

		let mut applied = Vec::with_capacity(step.undo.len());
		for tx in step.undo.iter().rev() {
			tx.apply(content);
			applied.push(tx.clone());
		}

		self.head_pos -= tx_len;
		self.redo_bytes += step.bytes;
		self.redo_tx_count += tx_len;
		self.redo_stack.push_back(step);
		self.enforce_limits();

		#[cfg(debug_assertions)]
		self.assert_invariants();

		Some(applied)
	}

	/// Redoes the last undone change by applying the sequence of forward transactions.
	///
	/// Returns the applied transactions in forward order.
	pub fn redo(&mut self, content: &mut Rope) -> Option<Vec<Transaction>> {
		self.active_group_owner = None;
		let step = self.redo_stack.pop_back()?;
		let tx_len = step.redo.len() as u64;
		self.redo_bytes = self.redo_bytes.saturating_sub(step.bytes);
		self.redo_tx_count -= tx_len;

		for tx in &step.redo {
			tx.apply(content);
		}

		self.head_pos += tx_len;
		let redo_txs = step.redo.clone();
		self.undo_bytes += step.bytes;
		self.undo_tx_count += tx_len;
		self.undo_stack.push_back(step);
		self.enforce_limits();

		#[cfg(debug_assertions)]
		self.assert_invariants();

		Some(redo_txs)
	}

	/// Clears all undo and redo history.
	pub fn clear_all(&mut self) {
		self.undo_stack.clear();
		self.redo_stack.clear();
		self.undo_bytes = 0;
		self.redo_bytes = 0;
		self.undo_tx_count = 0;
		self.redo_tx_count = 0;
		self.head_pos = 0;
		self.clean_pos = None;
		self.active_group_owner = None;

		#[cfg(debug_assertions)]
		self.assert_invariants();
	}

	/// Returns true if the current state is modified relative to the clean state.
	pub fn is_modified(&self) -> bool {
		self.clean_pos != Some(self.head_pos)
	}

	/// Marks the current state as clean.
	pub fn set_modified(&mut self, modified: bool) {
		if modified {
			self.clean_pos = None;
		} else {
			self.clean_pos = Some(self.head_pos);
		}
	}

	#[cfg(debug_assertions)]
	fn assert_invariants(&self) {
		let actual_undo_bytes: usize = self.undo_stack.iter().map(|s| s.bytes).sum();
		let actual_redo_bytes: usize = self.redo_stack.iter().map(|s| s.bytes).sum();
		let actual_undo_tx_count: u64 = self.undo_stack.iter().map(|s| s.undo.len() as u64).sum();
		let actual_redo_tx_count: u64 = self.redo_stack.iter().map(|s| s.redo.len() as u64).sum();

		assert_eq!(self.undo_bytes, actual_undo_bytes, "undo_bytes mismatch");
		assert_eq!(self.redo_bytes, actual_redo_bytes, "redo_bytes mismatch");
		assert_eq!(
			self.undo_tx_count, actual_undo_tx_count,
			"undo_tx_count mismatch"
		);
		assert_eq!(
			self.redo_tx_count, actual_redo_tx_count,
			"redo_tx_count mismatch"
		);
		assert!(
			self.undo_bytes + self.redo_bytes <= MAX_UNDO_BYTES,
			"total memory usage exceeds limit"
		);

		if let Some(cp) = self.clean_pos {
			let min_undo_pos = self.head_pos.saturating_sub(self.undo_tx_count);
			let max_redo_pos = self.head_pos + self.redo_tx_count;
			assert!(
				cp >= min_undo_pos && cp <= max_redo_pos,
				"clean_pos {} outside reachable history [{}, {}]",
				cp,
				min_undo_pos,
				max_redo_pos
			);
		}
	}
}

/// Unified undo backend for a single document.
///
/// Wraps a [`TxnUndoStore`] and provides a high-level API for recording commits
/// and performing undo/redo operations with document version updates.
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
	/// If `undo_policy` allows merging and the origin view matches the current
	/// group owner, it appends to the current group.
	pub fn record_commit(
		&mut self,
		tx: &Transaction,
		before: &Rope,
		undo_policy: UndoPolicy,
		origin_view: Option<ViewId>,
	) -> bool {
		let undo_tx = tx.invert(before);
		let merge = match undo_policy {
			UndoPolicy::MergeWithCurrentGroup => {
				self.store.active_group_owner == origin_view && origin_view.is_some()
			}
			_ => false,
		};

		let recorded = self.store.record_transaction(tx.clone(), undo_tx, merge);

		// Single-writer rule: only UndoBackend mutates active_group_owner.
		// Store-level methods (undo/redo/clear_all) reset it, but never set it.
		if matches!(
			undo_policy,
			UndoPolicy::MergeWithCurrentGroup | UndoPolicy::Boundary
		) {
			self.store.active_group_owner = origin_view;
		} else {
			self.store.active_group_owner = None;
		}

		recorded
	}

	/// Clears the active undo group owner.
	pub fn clear_active_group_owner(&mut self) {
		self.store.active_group_owner = None;
	}

	/// Performs undo, updating the document content and version.
	///
	/// Restores document state from the undo stack and increments the version.
	/// Returns the applied inverse transactions if undo was performed.
	pub fn undo(&mut self, content: &mut Rope, version: &mut u64) -> Option<Vec<Transaction>> {
		let applied = self.store.undo(content)?;
		*version = version.checked_add(1).expect("document version overflow");
		Some(applied)
	}

	/// Performs redo, updating the document content and version.
	///
	/// Restores document state from the redo stack and increments the version.
	/// Returns the applied forward transactions if redo was performed.
	pub fn redo(&mut self, content: &mut Rope, version: &mut u64) -> Option<Vec<Transaction>> {
		let applied = self.store.redo(content)?;
		*version = version.checked_add(1).expect("document version overflow");
		Some(applied)
	}

	/// Clears all undo and redo history.
	pub fn clear_all(&mut self) {
		self.store.clear_all();
	}

	/// Returns true if the current state is modified relative to the clean state.
	pub fn is_modified(&self) -> bool {
		self.store.is_modified()
	}

	/// Sets the modified flag.
	pub fn set_modified(&mut self, modified: bool) {
		self.store.set_modified(modified);
	}
}
