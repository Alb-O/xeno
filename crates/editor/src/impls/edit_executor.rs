//! Centralized edit executor for all text modification operations.
//!
//! [`EditExecutor`] provides a single entry point for all edit operations,
//! wrapping the Editor and ensuring consistent undo handling through
//! the [`UndoManager`] prepare/finalize pattern.
//!
//! # Usage
//!
//! ```ignore
//! let mut executor = editor.edit_executor();
//! executor.apply_transaction(buffer_id, &tx, Some(selection), policy);
//! executor.execute_edit_op(&edit_op);
//! executor.paste(false); // paste after
//! ```
//!
//! [`UndoManager`]: crate::types::UndoManager

use xeno_primitives::range::Direction as MoveDir;
use xeno_primitives::Selection;
use xeno_primitives::Transaction;
use xeno_registry::edit_op::EditOp;

use super::Editor;
use crate::buffer::BufferId;
use crate::types::ApplyEditPolicy;

/// Centralized executor for all edit operations.
///
/// Wraps a mutable reference to the Editor and provides methods for
/// applying transactions, executing edit operations, and paste operations.
/// All methods go through the [`UndoManager`] prepare/finalize pattern
/// for consistent undo handling.
///
/// [`UndoManager`]: crate::types::UndoManager
pub struct EditExecutor<'a> {
	editor: &'a mut Editor,
}

impl<'a> EditExecutor<'a> {
	/// Creates a new edit executor wrapping the given editor.
	pub fn new(editor: &'a mut Editor) -> Self {
		Self { editor }
	}

	/// Returns the focused buffer ID.
	pub fn focused_buffer(&self) -> BufferId {
		self.editor.focused_view()
	}

	/// Applies a transaction with the given policy.
	///
	/// This is the primary method for applying text changes. It:
	/// 1. Prepares the edit via `UndoManager` (captures view snapshots)
	/// 2. Applies the transaction
	/// 3. Finalizes via `UndoManager` (pushes `EditorUndoGroup` if needed)
	pub fn apply_transaction(
		&mut self,
		buffer_id: BufferId,
		tx: &Transaction,
		new_selection: Option<Selection>,
		policy: ApplyEditPolicy,
	) -> bool {
		self.editor
			.apply_edit(buffer_id, tx, new_selection, policy.undo, policy.origin)
	}

	/// Executes a data-oriented edit operation.
	///
	/// Compiles the `EditOp` into an `EditPlan` with resolved policies,
	/// then executes it. The undo policy is determined by the operation's
	/// transform type (e.g., Delete uses Record, Undo uses NoUndo).
	pub fn execute_edit_op(&mut self, op: &EditOp) {
		self.editor.execute_edit_op(op.clone());
	}

	/// Pastes from the yank register.
	///
	/// If `before` is true, pastes before the cursor; otherwise after.
	pub fn paste(&mut self, before: bool) {
		if before {
			self.editor.paste_before();
		} else {
			self.editor.paste_after();
		}
	}

	/// Moves the cursor visually (handling wrapped lines).
	pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
		self.editor.move_visual_vertical(direction, count, extend);
	}

	/// Checks if the buffer is readonly.
	pub fn is_readonly(&self) -> bool {
		self.editor.buffer().is_readonly()
	}

	/// Returns the underlying editor reference.
	///
	/// Use sparingly - prefer the executor's methods for edit operations.
	pub fn editor(&mut self) -> &mut Editor {
		self.editor
	}
}

impl Editor {
	/// Creates an edit executor for this editor.
	///
	/// The executor provides a unified entry point for all edit operations
	/// with consistent undo handling.
	pub fn edit_executor(&mut self) -> EditExecutor<'_> {
		EditExecutor::new(self)
	}
}
