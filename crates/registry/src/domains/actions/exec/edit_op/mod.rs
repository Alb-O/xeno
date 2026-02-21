//! Data-oriented edit operations.
//!
//! This module provides composable, data-driven text editing operations.
//! Operations are expressed as data records that describe:
//!
//! 1. How to modify the selection before editing
//! 2. What text transformation to apply
//! 3. What effects to apply after the edit
//!
//! # Compilation
//!
//! Edit operations can be compiled into an [`EditPlan`] which resolves policies
//! and validates the operation before execution. This enables the executor to
//! use `Document::commit()` with proper undo/syntax policies instead of having
//! sprinkled `save_undo_state()` calls in each transform.
//!
//! # Example
//!
//! ```ignore
//! use crate::edit_op::{EditOp, TextTransform, PostEffect};
//!
//! // Delete with yank - composable data
//! let delete_yank = EditOp::default()
//!     .with_pre(PreEffect::Yank)
//!     .with_transform(TextTransform::Delete);
//!
//! // Change operation = delete + insert mode
//! let change = EditOp::default()
//!     .with_pre(PreEffect::Yank)
//!     .with_transform(TextTransform::Delete)
//!     .with_post(PostEffect::SetMode(Mode::Insert));
//!
//! // Compile for execution with resolved policies
//! let plan = change.compile();
//! ```

use xeno_primitives::{Direction, EditOrigin, Mode, SyntaxPolicy, UndoPolicy};

/// A data description of a text edit operation.
///
/// Edit operations are composable: selection modification, text transformation,
/// and post-effects can be combined freely. The executor processes these
/// records uniformly, eliminating per-operation match arms.
#[derive(Debug, Clone, Default)]
pub struct EditOp {
	/// Effects to apply before the edit (e.g., yank, save undo).
	pub pre: Vec<PreEffect>,
	/// How to expand/modify selection before editing.
	pub selection: SelectionOp,
	/// The text transformation to apply.
	pub transform: TextTransform,
	/// Effects to apply after the edit.
	pub post: Vec<PostEffect>,
}

impl EditOp {
	/// Creates a new empty edit operation.
	#[inline]
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds a pre-effect.
	#[inline]
	pub fn with_pre(mut self, effect: PreEffect) -> Self {
		self.pre.push(effect);
		self
	}

	/// Sets the selection operation.
	#[inline]
	pub fn with_selection(mut self, op: SelectionOp) -> Self {
		self.selection = op;
		self
	}

	/// Sets the text transformation.
	#[inline]
	pub fn with_transform(mut self, transform: TextTransform) -> Self {
		self.transform = transform;
		self
	}

	/// Adds a post-effect.
	#[inline]
	pub fn with_post(mut self, effect: PostEffect) -> Self {
		self.post.push(effect);
		self
	}

	/// Returns true if this operation modifies text.
	#[inline]
	pub fn modifies_text(&self) -> bool {
		!matches!(self.transform, TextTransform::None)
	}

	/// Compiles this edit operation into an execution plan.
	///
	/// The plan resolves undo and syntax policies based on the transform type,
	/// enabling the executor to use `Document::commit()` with proper policies.
	///
	/// # Policy Resolution
	///
	/// * Text-modifying transforms (`Delete`, `Replace`, `Insert`, etc.): `UndoPolicy::Record`
	/// * `Undo`/`Redo` transforms: `UndoPolicy::NoUndo` (undo system handles this)
	/// * Non-modifying transforms (`None`): `UndoPolicy::NoUndo`
	/// * All text-modifying transforms use `SyntaxPolicy::MarkDirty` (lazy reparse)
	pub fn compile(&self) -> EditPlan {
		let (undo_policy, syntax_policy) = match &self.transform {
			TextTransform::None => (UndoPolicy::NoUndo, SyntaxPolicy::None),
			TextTransform::Undo | TextTransform::Redo => (UndoPolicy::NoUndo, SyntaxPolicy::FullReparseNow),
			TextTransform::Delete
			| TextTransform::Replace(_)
			| TextTransform::Insert(_)
			| TextTransform::InsertNewlineWithIndent
			| TextTransform::MapChars(_)
			| TextTransform::ReplaceEachChar(_)
			| TextTransform::Deindent { .. } => (UndoPolicy::Record, SyntaxPolicy::MarkDirty),
		};

		let origin = self.derive_origin();

		EditPlan {
			op: self.clone(),
			undo_policy,
			syntax_policy,
			origin,
		}
	}

	/// Derives an edit origin from the operation for grouping/telemetry.
	fn derive_origin(&self) -> EditOrigin {
		let id = match &self.transform {
			TextTransform::None => "none",
			TextTransform::Delete => "delete",
			TextTransform::Replace(_) => "replace",
			TextTransform::Insert(_) => "insert",
			TextTransform::InsertNewlineWithIndent => "newline",
			TextTransform::MapChars(_) => "case",
			TextTransform::ReplaceEachChar(_) => "replace_char",
			TextTransform::Undo => "undo",
			TextTransform::Redo => "redo",
			TextTransform::Deindent { .. } => "deindent",
		};
		EditOrigin::EditOp { id }
	}
}

/// A compiled edit plan ready for execution.
///
/// Created by [`EditOp::compile()`], this contains the original operation
/// plus resolved policies for undo recording and syntax updates.
#[derive(Debug, Clone)]
pub struct EditPlan {
	/// The original edit operation.
	pub op: EditOp,
	/// Resolved undo recording policy.
	pub undo_policy: UndoPolicy,
	/// Resolved syntax update policy.
	pub syntax_policy: SyntaxPolicy,
	/// Origin for grouping and telemetry.
	pub origin: EditOrigin,
}

/// Effects to apply before the main edit transformation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreEffect {
	/// Yank the current selection before modifying.
	Yank,
}

/// Selection modification before edit.
///
/// These operations adjust the selection before the text transformation
/// is applied. For example, "open below" first moves to line end.
#[derive(Debug, Clone, Default)]
pub enum SelectionOp {
	/// No selection modification.
	#[default]
	None,
	/// Extend selection in direction by count characters.
	Extend {
		/// Direction to extend.
		direction: Direction,
		/// Number of characters.
		count: usize,
	},
	/// Move all selections to line start.
	ToLineStart,
	/// Move all selections to line end.
	ToLineEnd,
	/// Expand selection to include full lines.
	ExpandToFullLines,
	/// Select the character before cursor.
	///
	/// Creates a collapsed selection at `head - 1`. Combined with [`Transaction::delete`]
	/// (which is inclusive of the head cell), this deletes exactly `[head-1, head)`.
	///
	/// [`Transaction::delete`]: xeno_primitives::Transaction::delete
	SelectCharBefore,
	/// Select the character after cursor.
	///
	/// Creates a collapsed selection at `head`. Combined with [`Transaction::delete`]
	/// (which is inclusive of the head cell), this deletes exactly `[head, head+1)`.
	///
	/// [`Transaction::delete`]: xeno_primitives::Transaction::delete
	SelectCharAfter,
	/// Select from cursor back to previous word start.
	SelectWordBefore,
	/// Select from cursor forward to next word end.
	SelectWordAfter,
	/// Select from current position to next line start.
	SelectToNextLineStart,
	/// Position cursor after current position.
	PositionAfterCursor,
}

/// Text transformation applied to selection.
///
/// These are the primitive text operations. Complex operations are built
/// by combining transformations with pre/post effects.
#[derive(Debug, Clone, Default)]
pub enum TextTransform {
	/// No text change.
	#[default]
	None,
	/// Delete selected text.
	Delete,
	/// Replace selected text with literal content.
	Replace(String),
	/// Insert text at cursor (selection becomes empty, text inserted).
	Insert(String),
	/// Insert newline with indentation copied from current line.
	InsertNewlineWithIndent,
	/// Apply character mapping (case conversion).
	MapChars(CharMapKind),
	/// Replace each character in selection with the given char (vim's r).
	ReplaceEachChar(char),
	/// Undo last change (special operation).
	Undo,
	/// Redo last undone change (special operation).
	Redo,
	/// Deindent by up to N spaces (special operation with space detection).
	Deindent { max_spaces: usize },
}

/// Character mapping operations for case conversion.
///
/// Using an enum instead of a function pointer for Clone + Debug + Eq.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharMapKind {
	/// Convert to lowercase.
	ToLowerCase,
	/// Convert to uppercase.
	ToUpperCase,
	/// Swap case (upper <-> lower).
	SwapCase,
}

impl CharMapKind {
	/// Applies the character mapping to a single character.
	pub fn apply(self, c: char) -> impl Iterator<Item = char> {
		match self {
			Self::ToLowerCase => CharMapIter::Lower(c.to_lowercase()),
			Self::ToUpperCase => CharMapIter::Upper(c.to_uppercase()),
			Self::SwapCase => {
				if c.is_uppercase() {
					CharMapIter::Lower(c.to_lowercase())
				} else {
					CharMapIter::Upper(c.to_uppercase())
				}
			}
		}
	}
}

/// Iterator for character mapping results.
enum CharMapIter {
	Lower(std::char::ToLowercase),
	Upper(std::char::ToUppercase),
}

impl Iterator for CharMapIter {
	type Item = char;

	fn next(&mut self) -> Option<Self::Item> {
		match self {
			Self::Lower(iter) => iter.next(),
			Self::Upper(iter) => iter.next(),
		}
	}
}

/// Post-edit effects.
///
/// Applied after the text transformation completes.
#[derive(Debug, Clone, PartialEq)]
pub enum PostEffect {
	/// Change editor mode.
	SetMode(Mode),
	/// Move cursor relative to edit result.
	MoveCursor(CursorAdjust),
}

/// Cursor adjustment after edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorAdjust {
	/// Move cursor up by count lines.
	Up(usize),
	/// Keep cursor at current position (default).
	#[default]
	Stay,
	/// Move to start of inserted/modified text.
	ToStart,
	/// Move to end of inserted/modified text.
	ToEnd,
}

/// Creates a delete operation.
///
/// # Arguments
/// * `yank` - If true, yank selection before deleting.
pub fn delete(yank: bool) -> EditOp {
	let mut op = EditOp::new().with_transform(TextTransform::Delete);
	if yank {
		op = op.with_pre(PreEffect::Yank);
	}
	op
}

/// Creates a change operation (delete + enter insert mode).
///
/// # Arguments
/// * `yank` - If true, yank selection before deleting.
pub fn change(yank: bool) -> EditOp {
	let mut op = EditOp::new().with_transform(TextTransform::Delete).with_post(PostEffect::SetMode(Mode::Insert));
	if yank {
		op = op.with_pre(PreEffect::Yank);
	}
	op
}

/// Creates a yank operation (copy without delete).
pub fn yank() -> EditOp {
	EditOp::new().with_pre(PreEffect::Yank)
}

/// Creates a replace-with-char operation (vim's r).
///
/// Replaces each character in selection with the given character.
pub fn replace_with_char(ch: char) -> EditOp {
	EditOp::new().with_transform(TextTransform::ReplaceEachChar(ch))
}

/// Creates an undo operation.
pub fn undo() -> EditOp {
	EditOp::new().with_transform(TextTransform::Undo)
}

/// Creates a redo operation.
pub fn redo() -> EditOp {
	EditOp::new().with_transform(TextTransform::Redo)
}

/// Creates an indent operation.
pub fn indent() -> EditOp {
	EditOp::new()
		.with_selection(SelectionOp::ToLineStart)
		.with_transform(TextTransform::Insert("    ".to_string()))
}

/// Creates a deindent operation.
///
/// Removes up to 4 leading spaces from the current line.
pub fn deindent() -> EditOp {
	EditOp::new().with_transform(TextTransform::Deindent { max_spaces: 4 })
}

/// Creates a case conversion operation.
pub fn case_convert(kind: CharMapKind) -> EditOp {
	EditOp::new().with_transform(TextTransform::MapChars(kind))
}

/// Creates a join-lines operation.
pub fn join_lines() -> EditOp {
	EditOp::new()
		.with_selection(SelectionOp::SelectToNextLineStart)
		.with_transform(TextTransform::Replace(" ".to_string()))
}

/// Creates a delete-back (backspace) operation.
pub fn delete_back() -> EditOp {
	EditOp::new()
		.with_selection(SelectionOp::SelectCharBefore)
		.with_transform(TextTransform::Delete)
}

/// Creates a delete-forward (delete key) operation.
pub fn delete_forward() -> EditOp {
	EditOp::new().with_selection(SelectionOp::SelectCharAfter).with_transform(TextTransform::Delete)
}

/// Creates a delete-word-back operation.
pub fn delete_word_back() -> EditOp {
	EditOp::new()
		.with_selection(SelectionOp::SelectWordBefore)
		.with_transform(TextTransform::Delete)
}

/// Creates a delete-word-forward operation.
pub fn delete_word_forward() -> EditOp {
	EditOp::new().with_selection(SelectionOp::SelectWordAfter).with_transform(TextTransform::Delete)
}

/// Creates a newline insertion with smart indentation.
pub fn insert_newline() -> EditOp {
	EditOp::new().with_transform(TextTransform::InsertNewlineWithIndent)
}

/// Creates an open-below operation (new line below, enter insert).
pub fn open_below() -> EditOp {
	EditOp::new()
		.with_selection(SelectionOp::ToLineEnd)
		.with_transform(TextTransform::InsertNewlineWithIndent)
		.with_post(PostEffect::SetMode(Mode::Insert))
}

/// Creates an open-above operation (new line above, enter insert).
pub fn open_above() -> EditOp {
	EditOp::new()
		.with_selection(SelectionOp::ToLineStart)
		.with_transform(TextTransform::InsertNewlineWithIndent)
		.with_post(PostEffect::MoveCursor(CursorAdjust::Up(1)))
		.with_post(PostEffect::SetMode(Mode::Insert))
}

/// Creates an add-line-below operation (blank line, stay in normal).
pub fn add_line_below() -> EditOp {
	EditOp::new()
		.with_selection(SelectionOp::ToLineEnd)
		.with_transform(TextTransform::Insert("\n".to_string()))
		.with_post(PostEffect::MoveCursor(CursorAdjust::Stay))
}

/// Creates an add-line-above operation (blank line, stay in normal).
pub fn add_line_above() -> EditOp {
	EditOp::new()
		.with_selection(SelectionOp::ToLineStart)
		.with_transform(TextTransform::Insert("\n".to_string()))
		.with_post(PostEffect::MoveCursor(CursorAdjust::Stay))
}

#[cfg(test)]
mod tests;
