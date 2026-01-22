//! Data-oriented edit operation executor.
//!
//! This module provides a single executor function that processes [`EditOp`]
//! records. All text editing operations are expressed as data and processed
//! uniformly by this executor.
//!
//! # Compile -> Commit Pattern
//!
//! The executor compiles EditOp into an EditPlan with resolved policies, then
//! builds a transaction and applies it via `apply_edit()`. This ensures:
//!
//! - Undo recording happens inside `commit()` with proper transaction context
//! - View snapshots are captured by `apply_edit()` before the transaction
//! - Syntax updates use the lazy `MarkDirty` policy by default
//!
//! # Execution Phases
//!
//! 1. Compile: Resolve undo/syntax policies based on transform type
//! 2. Pre-effects: Execute yank, extend selection
//! 3. Selection modification: Adjust selection before transform
//! 4. Build transaction: Create transaction from the transform
//! 5. Apply: Call `apply_edit()` which handles undo and commits the transaction
//! 6. Post-effects: Mode change, cursor adjustment

use xeno_primitives::range::{Direction as MoveDir, Range};
use xeno_primitives::transaction::Change;
use xeno_primitives::{EditOrigin, Selection, Transaction, UndoPolicy};
use xeno_registry::ModeAccess;
use xeno_registry::edit_op::{
	CharMapKind, CursorAdjust, EditOp, EditPlan, PostEffect, PreEffect, SelectionOp, TextTransform,
};

use super::Editor;
use crate::movement::{self, WordType};

impl Editor {
	/// Executes a data-oriented edit operation.
	///
	/// Compiles the operation into an [`EditPlan`] with resolved policies,
	/// then executes it using the compile -> commit pattern.
	pub fn execute_edit_op(&mut self, op: EditOp) {
		let plan = op.compile();
		self.execute_edit_plan(plan);
	}

	/// Executes a compiled edit plan.
	///
	/// Uses the compile -> commit pattern:
	/// 1. Check readonly and apply pre-effects/selection ops
	/// 2. Build the transaction from the transform
	/// 3. Apply via `apply_edit()` which handles undo recording inside commit
	/// 4. Apply post-effects
	pub fn execute_edit_plan(&mut self, plan: EditPlan) {
		if plan.op.modifies_text() && !self.guard_readonly() {
			return;
		}

		for pre in &plan.op.pre {
			self.apply_pre_effect(pre);
		}

		if !self.apply_selection_op(&plan.op.selection) {
			return;
		}

		let original_cursor = self.buffer().cursor;

		if let Some((tx, new_selection)) = self.build_transform_transaction(&plan) {
			let buffer_id = self.focused_view();
			self.apply_edit(
				buffer_id,
				&tx,
				Some(new_selection),
				plan.undo_policy,
				EditOrigin::Internal("edit_op"),
			);
		}

		for post in &plan.op.post {
			self.apply_post_effect(post, original_cursor);
		}
	}

	/// Applies a pre-effect before the main transformation.
	fn apply_pre_effect(&mut self, effect: &PreEffect) {
		match effect {
			PreEffect::Yank => {
				self.yank_selection();
			}
		}
	}

	/// Applies a selection operation before text transformation.
	///
	/// Returns `false` if the operation produced no valid targets (e.g., backspace
	/// at document start), signaling that the transform should be skipped.
	fn apply_selection_op(&mut self, op: &SelectionOp) -> bool {
		match op {
			SelectionOp::None => true,

			SelectionOp::Extend { direction, count } => {
				let new_ranges: Vec<_> = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								movement::move_horizontally(
									doc.content().slice(..),
									*r,
									*direction,
									*count,
									true,
								)
							})
							.collect()
					})
				};
				let primary_index = self.buffer().selection.primary_index();
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
				true
			}

			SelectionOp::ToLineStart => {
				let (new_ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let ranges: Vec<_> = buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								movement::move_to_line_start(doc.content().slice(..), *r, false)
							})
							.collect();
						(ranges, buffer.selection.primary_index())
					})
				};
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
				true
			}

			SelectionOp::ToLineEnd => {
				let (new_ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let ranges: Vec<_> = buffer
							.selection
							.ranges()
							.iter()
							.map(|r| movement::move_to_line_end(doc.content().slice(..), *r, false))
							.collect();
						(ranges, buffer.selection.primary_index())
					})
				};
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
				true
			}

			SelectionOp::ExpandToFullLines => {
				let (new_ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let ranges: Vec<_> = buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								let start_line = doc.content().char_to_line(r.from());
								let end_line = doc.content().char_to_line(r.to());
								let start = doc.content().line_to_char(start_line);
								let end = if end_line + 1 < doc.content().len_lines() {
									doc.content().line_to_char(end_line + 1)
								} else {
									doc.content().len_chars()
								};
								Range::new(start, end)
							})
							.collect();
						(ranges, buffer.selection.primary_index())
					})
				};
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
				true
			}

			SelectionOp::SelectCharBefore => {
				let buffer = self.buffer();
				let primary_sel_idx = buffer.selection.primary_index();
				let mut ranges = Vec::new();
				let mut primary_index = 0usize;
				for (idx, range) in buffer.selection.ranges().iter().enumerate() {
					if range.head == 0 {
						continue;
					}
					if idx == primary_sel_idx {
						primary_index = ranges.len();
					}
					let pos = range.head - 1;
					ranges.push(Range::new(pos, pos));
				}
				if ranges.is_empty() {
					return false;
				}
				self.buffer_mut()
					.set_selection(Selection::from_vec(ranges, primary_index));
				true
			}

			SelectionOp::SelectCharAfter => {
				let (ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let len = doc.content().len_chars();
						let primary_sel_idx = buffer.selection.primary_index();
						let mut ranges = Vec::new();
						let mut primary_index = 0usize;
						for (idx, range) in buffer.selection.ranges().iter().enumerate() {
							if range.head >= len {
								continue;
							}
							if idx == primary_sel_idx {
								primary_index = ranges.len();
							}
							ranges.push(Range::new(range.head, range.head));
						}
						(ranges, primary_index)
					})
				};
				if ranges.is_empty() {
					return false;
				}
				self.buffer_mut()
					.set_selection(Selection::from_vec(ranges, primary_index));
				true
			}

			SelectionOp::SelectWordBefore => {
				let (ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let text = doc.content().slice(..);
						let primary_sel_idx = buffer.selection.primary_index();
						let mut ranges = Vec::new();
						let mut primary_index = 0usize;
						for (idx, range) in buffer.selection.ranges().iter().enumerate() {
							if range.head == 0 {
								continue;
							}
							if idx == primary_sel_idx {
								primary_index = ranges.len();
							}
							let word_start = movement::move_to_prev_word_start(
								text,
								*range,
								1,
								WordType::Word,
								false,
							);
							if range.head > word_start.head {
								ranges.push(Range::new(word_start.head, range.head - 1));
							}
						}
						(ranges, primary_index)
					})
				};
				if ranges.is_empty() {
					return false;
				}
				self.buffer_mut()
					.set_selection(Selection::from_vec(ranges, primary_index));
				true
			}

			SelectionOp::SelectWordAfter => {
				let (ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let text = doc.content().slice(..);
						let len = text.len_chars();
						let primary_sel_idx = buffer.selection.primary_index();
						let mut ranges = Vec::new();
						let mut primary_index = 0usize;
						for (idx, range) in buffer.selection.ranges().iter().enumerate() {
							if range.head >= len {
								continue;
							}
							if idx == primary_sel_idx {
								primary_index = ranges.len();
							}
							let word_end = movement::move_to_next_word_start(
								text,
								*range,
								1,
								WordType::Word,
								false,
							);
							if word_end.head > range.head {
								ranges.push(Range::new(range.head, word_end.head - 1));
							}
						}
						(ranges, primary_index)
					})
				};
				if ranges.is_empty() {
					return false;
				}
				self.buffer_mut()
					.set_selection(Selection::from_vec(ranges, primary_index));
				true
			}

			SelectionOp::SelectToNextLineStart => {
				let selection = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let primary = buffer.selection.primary();
						let line = doc.content().char_to_line(primary.head);
						let total_lines = doc.content().len_lines();
						if line + 1 < total_lines {
							let end_of_line = doc.content().line_to_char(line + 1) - 1;
							Some(Selection::single(end_of_line, end_of_line + 1))
						} else {
							None
						}
					})
				};
				match selection {
					Some(sel) => {
						self.buffer_mut().set_selection(sel);
						true
					}
					None => false,
				}
			}

			SelectionOp::PositionAfterCursor => {
				let new_ranges: Vec<_> = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let len = doc.content().len_chars();
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								let new_pos = (r.head + 1).min(len);
								Range::point(new_pos)
							})
							.collect()
					})
				};
				let primary_index = self.buffer().selection.primary_index();
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
				true
			}
		}
	}

	/// Builds a transaction for the text transformation in the plan.
	///
	/// Returns `None` for transforms that don't produce a transaction (None, Undo, Redo)
	/// or when the transform has nothing to do (e.g., delete on empty selection).
	///
	/// Meta transforms (Undo/Redo) are executed directly since they don't produce
	/// a transaction that can be composed with apply_edit.
	fn build_transform_transaction(&mut self, plan: &EditPlan) -> Option<(Transaction, Selection)> {
		match &plan.op.transform {
			TextTransform::None => None,
			TextTransform::Delete => self.build_delete_transaction(),
			TextTransform::Replace(text) => self.build_replace_transaction(text),
			TextTransform::Insert(text) => self.build_insert_transaction(text),
			TextTransform::InsertNewlineWithIndent => self.build_newline_with_indent_transaction(),
			TextTransform::MapChars(kind) => self.build_char_mapping_transaction(*kind),
			TextTransform::ReplaceEachChar(ch) => self.build_replace_each_char_transaction(*ch),
			TextTransform::Deindent { max_spaces } => self.build_deindent_transaction(*max_spaces),
			TextTransform::Undo => {
				self.undo();
				None
			}
			TextTransform::Redo => {
				self.redo();
				None
			}
		}
	}

	/// Applies a post-effect after the main transformation.
	///
	/// For [`CursorAdjust::ToStart`] and [`CursorAdjust::ToEnd`], the cursor is already
	/// positioned by the insert operation, so no action is needed.
	fn apply_post_effect(&mut self, effect: &PostEffect, original_cursor: usize) {
		match effect {
			PostEffect::SetMode(mode) => {
				self.set_mode(mode.clone());
			}

			PostEffect::MoveCursor(adjust) => match adjust {
				CursorAdjust::Stay => {
					let len = self.buffer().with_doc(|doc| doc.content().len_chars());
					let pos = original_cursor.min(len.saturating_sub(1));
					self.buffer_mut()
						.set_cursor_and_selection(pos, Selection::point(pos));
				}
				CursorAdjust::Up(count) => {
					let (new_ranges, primary_index) = {
						let buffer = self.buffer();
						buffer.with_doc(|doc| {
							let ranges: Vec<_> = buffer
								.selection
								.ranges()
								.iter()
								.map(|r| {
									movement::move_vertically(
										doc.content().slice(..),
										*r,
										MoveDir::Backward,
										*count,
										false,
									)
								})
								.collect();
							(ranges, buffer.selection.primary_index())
						})
					};
					self.buffer_mut()
						.set_selection(Selection::from_vec(new_ranges, primary_index));
				}
				CursorAdjust::ToStart | CursorAdjust::ToEnd => {}
			},
		}
	}

	/// Builds a delete transaction for the current selection.
	///
	/// For empty selections (cursor only), deletes the character at the cursor
	/// position since `to_inclusive()` ensures the cursor is always included.
	fn build_delete_transaction(&self) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer();
		buffer.with_doc(|doc| {
			let tx = Transaction::delete(doc.content().slice(..), &buffer.selection);
			let new_sel = tx.map_selection(&buffer.selection);
			Some((tx, new_sel))
		})
	}

	/// Builds an insert transaction for the given text.
	fn build_insert_transaction(&mut self, text: &str) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer_mut();
		Some(buffer.prepare_insert(text))
	}

	/// Builds a newline+indent insert transaction.
	fn build_newline_with_indent_transaction(&mut self) -> Option<(Transaction, Selection)> {
		let indent = {
			let buffer = self.buffer();
			let cursor = buffer.cursor;
			buffer.with_doc(|doc| {
				let line_idx = doc.content().char_to_line(cursor);
				let line = doc.content().line(line_idx);
				line.chars()
					.take_while(|c| *c == ' ' || *c == '\t')
					.collect::<String>()
			})
		};
		let text = format!("\n{}", indent);
		self.build_insert_transaction(&text)
	}

	/// Builds a replace transaction (delete selection + insert replacement).
	///
	/// Uses [`Transaction::change`] to build a single transaction that replaces
	/// each selection range with the given text. If the selection is empty,
	/// falls back to a simple insert.
	fn build_replace_transaction(&mut self, replacement: &str) -> Option<(Transaction, Selection)> {
		if self.buffer().selection.primary().is_empty() {
			return self.build_insert_transaction(replacement);
		}
		let buffer = self.buffer();
		buffer.with_doc(|doc| {
			let replacement_str: String = replacement.into();
			let changes = buffer.selection.iter().map(|range| Change {
				start: range.from(),
				end: range.to_inclusive(),
				replacement: Some(replacement_str.clone()),
			});
			let tx = Transaction::change(doc.content().slice(..), changes);
			let new_sel = tx.map_selection(&buffer.selection);
			Some((tx, new_sel))
		})
	}

	/// Builds a character mapping transaction (e.g., uppercase, lowercase, swap case).
	fn build_char_mapping_transaction(
		&self,
		kind: CharMapKind,
	) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer();
		let primary = buffer.selection.primary();
		let from = primary.from();
		let to = primary.to_inclusive();
		if from >= to {
			return None;
		}

		buffer.with_doc(|doc| {
			let mapped: String = doc
				.content()
				.slice(from..to)
				.chars()
				.flat_map(|c| kind.apply(c))
				.collect();

			let changes = buffer.selection.iter().map(|range| Change {
				start: range.from(),
				end: range.to_inclusive(),
				replacement: Some(mapped.clone()),
			});
			let tx = Transaction::change(doc.content().slice(..), changes);
			let new_sel = tx.map_selection(&buffer.selection);
			Some((tx, new_sel))
		})
	}

	/// Builds a replace-each-char transaction (e.g., vim's `r` command).
	///
	/// For empty selections, replaces the single character at cursor position.
	fn build_replace_each_char_transaction(&self, ch: char) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer();
		let primary = buffer.selection.primary();
		let from = primary.from();
		let to = primary.to_inclusive();

		buffer.with_doc(|doc| {
			let len = if from < to { to - from } else { 1 };
			let replacement: String = std::iter::repeat_n(ch, len).collect();

			let selection = if from >= to {
				Selection::single(from, (from + 1).min(doc.content().len_chars()))
			} else {
				buffer.selection.clone()
			};

			let changes = selection.iter().map(|range| Change {
				start: range.from(),
				end: range.to_inclusive(),
				replacement: Some(replacement.clone()),
			});
			let tx = Transaction::change(doc.content().slice(..), changes);
			let new_sel = tx.map_selection(&selection);
			Some((tx, new_sel))
		})
	}

	/// Builds a deindent transaction.
	///
	/// Removes up to `max_spaces` leading spaces from the current line and
	/// adjusts cursor position accordingly. Returns `None` if there are no
	/// leading spaces to remove.
	fn build_deindent_transaction(&self, max_spaces: usize) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer();
		buffer.with_doc(|doc| {
			let line = doc.content().char_to_line(buffer.cursor);
			let line_start = doc.content().line_to_char(line);
			let line_text: String = doc.content().line(line).chars().take(max_spaces).collect();
			let spaces = line_text
				.chars()
				.take_while(|c| *c == ' ')
				.count()
				.min(max_spaces);

			if spaces == 0 {
				return None;
			}

			let delete_selection = Selection::single(line_start, line_start + spaces);
			let tx = Transaction::delete(doc.content().slice(..), &delete_selection);

			let old_cursor = buffer.cursor;
			let delete_end = line_start + spaces;
			let new_cursor = if old_cursor > delete_end {
				old_cursor.saturating_sub(spaces)
			} else if old_cursor > line_start {
				line_start
			} else {
				old_cursor
			};

			Some((tx, Selection::point(new_cursor)))
		})
	}

	/// Replaces the current selection with the given text.
	///
	/// This is a helper for text replacement operations.
	pub fn replace_selection(&mut self, text: &str) {
		if !self.guard_readonly() {
			return;
		}

		if let Some((tx, new_selection)) = self.build_replace_transaction(text) {
			let buffer_id = self.focused_view();
			self.apply_edit(
				buffer_id,
				&tx,
				Some(new_selection),
				UndoPolicy::Record,
				EditOrigin::Internal("replace_selection"),
			);
		}
	}
}
