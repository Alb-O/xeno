//! Data-oriented edit operation executor.
//!
//! This module provides a single executor function that processes [`EditOp`]
//! records. All text editing operations are expressed as data and processed
//! uniformly by this executor.
//!
//! The executor handles operations in phases:
//! 1. Pre-effects (yank, save undo state)
//! 2. Selection modification
//! 3. Text transformation
//! 4. Post-effects (mode change, cursor adjustment)

use xeno_base::range::{Direction as MoveDir, Range};
use xeno_base::{Selection, Transaction};
use xeno_core::editor_ctx::ModeAccess;
use xeno_core::movement;
use xeno_registry::edit_op::{
	CharMapKind, CursorAdjust, EditOp, PostEffect, PreEffect, SelectionOp, TextTransform,
};

use super::Editor;

impl Editor {
	/// Executes a data-oriented edit operation.
	///
	/// This is the single executor for all text editing operations.
	pub fn execute_edit_op(&mut self, op: EditOp) {
		for pre in &op.pre {
			self.apply_pre_effect(pre);
		}

		self.apply_selection_op(&op.selection);

		let original_cursor = self.buffer().cursor;
		self.apply_text_transform(&op.transform);

		for post in &op.post {
			self.apply_post_effect(post, original_cursor);
		}
	}

	/// Applies a pre-effect before the main transformation.
	fn apply_pre_effect(&mut self, effect: &PreEffect) {
		match effect {
			PreEffect::Yank => {
				self.yank_selection();
			}
			PreEffect::SaveUndo => {
				self.save_undo_state();
			}
			PreEffect::ExtendForwardIfEmpty => {
				if self.buffer().selection.primary().is_empty() {
					let new_ranges: Vec<_> = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								movement::move_horizontally(
									doc.content.slice(..),
									*r,
									MoveDir::Forward,
									1,
									true,
								)
							})
							.collect()
					};
					let primary_index = self.buffer().selection.primary_index();
					self.buffer_mut()
						.set_selection(Selection::from_vec(new_ranges, primary_index));
				}
			}
		}
	}

	/// Applies a selection operation before text transformation.
	fn apply_selection_op(&mut self, op: &SelectionOp) {
		match op {
			SelectionOp::None => {}

			SelectionOp::Extend { direction, count } => {
				let new_ranges: Vec<_> = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					buffer
						.selection
						.ranges()
						.iter()
						.map(|r| {
							movement::move_horizontally(
								doc.content.slice(..),
								*r,
								*direction,
								*count,
								true,
							)
						})
						.collect()
				};
				let primary_index = self.buffer().selection.primary_index();
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
			}

			SelectionOp::ToLineStart => {
				let (new_ranges, primary_index) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let ranges: Vec<_> = buffer
						.selection
						.ranges()
						.iter()
						.map(|r| movement::move_to_line_start(doc.content.slice(..), *r, false))
						.collect();
					(ranges, buffer.selection.primary_index())
				};
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
			}

			SelectionOp::ToLineEnd => {
				let (new_ranges, primary_index) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let ranges: Vec<_> = buffer
						.selection
						.ranges()
						.iter()
						.map(|r| movement::move_to_line_end(doc.content.slice(..), *r, false))
						.collect();
					(ranges, buffer.selection.primary_index())
				};
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
			}

			SelectionOp::ExpandToFullLines => {
				let (new_ranges, primary_index) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let ranges: Vec<_> = buffer
						.selection
						.ranges()
						.iter()
						.map(|r| {
							let start_line = doc.content.char_to_line(r.from());
							let end_line = doc.content.char_to_line(r.to());
							let start = doc.content.line_to_char(start_line);
							let end = if end_line + 1 < doc.content.len_lines() {
								doc.content.line_to_char(end_line + 1)
							} else {
								doc.content.len_chars()
							};
							Range::new(start, end)
						})
						.collect();
					(ranges, buffer.selection.primary_index())
				};
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
			}

			SelectionOp::SelectCharBefore => {
				let (ranges, primary_index) = {
					let buffer = self.buffer();
					let mut ranges = Vec::new();
					let mut primary_index = 0usize;
					for (idx, range) in buffer.selection.ranges().iter().enumerate() {
						if range.head == 0 {
							continue;
						}
						if idx == buffer.selection.primary_index() {
							primary_index = ranges.len();
						}
						ranges.push(Range::new(range.head - 1, range.head));
					}
					(ranges, primary_index)
				};

				if !ranges.is_empty() {
					self.buffer_mut()
						.set_selection(Selection::from_vec(ranges, primary_index));
				}
			}

			SelectionOp::SelectToNextLineStart => {
				let (selection, valid) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let primary = buffer.selection.primary();
					let line = doc.content.char_to_line(primary.head);
					let total_lines = doc.content.len_lines();
					if line + 1 < total_lines {
						let end_of_line = doc.content.line_to_char(line + 1) - 1;
						(Selection::single(end_of_line, end_of_line + 1), true)
					} else {
						(buffer.selection.clone(), false)
					}
				};
				if valid {
					self.buffer_mut().set_selection(selection);
				}
			}

			SelectionOp::PositionAfterCursor => {
				let new_ranges: Vec<_> = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let len = doc.content.len_chars();
					buffer
						.selection
						.ranges()
						.iter()
						.map(|r| {
							let new_pos = (r.head + 1).min(len);
							Range::point(new_pos)
						})
						.collect()
				};
				let primary_index = self.buffer().selection.primary_index();
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
			}
		}
	}

	/// Applies a text transformation to the current selection.
	fn apply_text_transform(&mut self, transform: &TextTransform) {
		match transform {
			TextTransform::None => {}

			TextTransform::Delete => {
				if self.buffer().selection.primary().is_empty() {
					return;
				}
				if !self.guard_readonly() {
					return;
				}
				self.save_undo_state();
				let (tx, new_sel) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
					let new_sel = tx.map_selection(&buffer.selection);
					(tx, new_sel)
				};
				self.buffer_mut().finalize_selection(new_sel);
				self.apply_transaction(&tx);
			}

			TextTransform::Replace(text) => {
				if !self.guard_readonly() {
					return;
				}
				self.save_undo_state();
				let (tx, new_sel) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
					let new_sel = tx.map_selection(&buffer.selection);
					(tx, new_sel)
				};
				self.buffer_mut().finalize_selection(new_sel);
				self.apply_transaction(&tx);
				self.insert_text(text);
			}

			TextTransform::Insert(text) => {
				self.insert_text(text);
			}

			TextTransform::MapChars(kind) => {
				self.apply_char_mapping(*kind);
			}

			TextTransform::ReplaceEachChar(ch) => {
				self.apply_replace_each_char(*ch);
			}

			TextTransform::Undo => {
				self.undo();
			}

			TextTransform::Redo => {
				self.redo();
			}

			TextTransform::Deindent { max_spaces } => {
				self.apply_deindent(*max_spaces);
			}
		}
	}

	/// Applies a character mapping transformation (case conversion).
	fn apply_char_mapping(&mut self, kind: CharMapKind) {
		let primary = self.buffer().selection.primary();
		let from = primary.from();
		let to = primary.to();
		if from >= to {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		self.save_undo_state();
		let text: String = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			doc.content
				.slice(from..to)
				.chars()
				.flat_map(|c| kind.apply(c))
				.collect()
		};
		let new_len = text.chars().count();

		let (tx, new_sel) = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
			let new_sel = tx.map_selection(&buffer.selection);
			(tx, new_sel)
		};
		self.buffer_mut().finalize_selection(new_sel);
		self.apply_transaction(&tx);

		let tx = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			Transaction::insert(doc.content.slice(..), &buffer.selection, text)
		};
		self.apply_transaction(&tx);

		let new_cursor = self.buffer().selection.primary().head + new_len;
		self.buffer_mut()
			.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
	}

	/// Applies a post-effect after the main transformation.
	fn apply_post_effect(&mut self, effect: &PostEffect, original_cursor: usize) {
		match effect {
			PostEffect::SetMode(mode) => {
				self.set_mode(mode.clone());
			}

			PostEffect::MoveCursor(adjust) => {
				match adjust {
					CursorAdjust::Stay => {
						let len = self.buffer().doc().content.len_chars();
						let pos = original_cursor.min(len.saturating_sub(1));
						self.buffer_mut()
							.set_cursor_and_selection(pos, Selection::point(pos));
					}
					CursorAdjust::Up(count) => {
						let (new_ranges, primary_index) = {
							let buffer = self.buffer();
							let doc = buffer.doc();
							let ranges: Vec<_> = buffer
								.selection
								.ranges()
								.iter()
								.map(|r| {
									movement::move_vertically(
										doc.content.slice(..),
										*r,
										MoveDir::Backward,
										*count,
										false,
									)
								})
								.collect();
							(ranges, buffer.selection.primary_index())
						};
						self.buffer_mut()
							.set_selection(Selection::from_vec(new_ranges, primary_index));
					}
					CursorAdjust::ToStart | CursorAdjust::ToEnd => {
						// Cursor is already positioned by the insert operation
					}
				}
			}
		}
	}

	/// Replaces the current selection with the given text.
	///
	/// This is a helper for text replacement operations.
	pub fn replace_selection(&mut self, text: &str) {
		if !self.guard_readonly() {
			return;
		}
		self.save_undo_state();

		if !self.buffer().selection.primary().is_empty() {
			let (tx, new_sel) = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
				let new_sel = tx.map_selection(&buffer.selection);
				(tx, new_sel)
			};
			self.buffer_mut().set_selection(new_sel);
			self.apply_transaction(&tx);
		}

		self.insert_text(text);
	}

	/// Replaces each character in the selection with the given character.
	///
	/// If selection is empty, replaces the character under cursor.
	fn apply_replace_each_char(&mut self, ch: char) {
		let primary = self.buffer().selection.primary();
		let from = primary.from();
		let to = primary.to();

		if from < to {
			if !self.guard_readonly() {
				return;
			}
			self.save_undo_state();
			let len = to - from;
			let replacement: String = std::iter::repeat_n(ch, len).collect();

			let (tx, new_sel) = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
				let new_sel = tx.map_selection(&buffer.selection);
				(tx, new_sel)
			};
			self.buffer_mut().finalize_selection(new_sel);
			self.apply_transaction(&tx);

			let tx = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				Transaction::insert(doc.content.slice(..), &buffer.selection, replacement)
			};
			self.apply_transaction(&tx);

			let new_cursor = self.buffer().selection.primary().head + len;
			self.buffer_mut()
				.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
		} else {
			// Empty selection - replace single character under cursor
			if !self.guard_readonly() {
				return;
			}
			self.save_undo_state();
			self.buffer_mut()
				.set_selection(Selection::single(from, from + 1));

			let (tx, new_sel) = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
				let new_sel = tx.map_selection(&buffer.selection);
				(tx, new_sel)
			};
			self.buffer_mut().finalize_selection(new_sel);
			self.apply_transaction(&tx);

			let tx = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				Transaction::insert(doc.content.slice(..), &buffer.selection, ch.to_string())
			};
			self.apply_transaction(&tx);

			let new_cursor = self.buffer().selection.primary().head + 1;
			self.buffer_mut()
				.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
		}
	}

	/// Removes up to `max_spaces` leading spaces from the current line.
	///
	/// Counts consecutive spaces at line start (up to `max_spaces`) and deletes them.
	/// The cursor is repositioned based on its original location relative to the
	/// deleted range: cursors after the range shift left, cursors within the range
	/// move to line start, and cursors before the range are unchanged.
	fn apply_deindent(&mut self, max_spaces: usize) {
		let (line_start, spaces, old_cursor) = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			let line = doc.content.char_to_line(buffer.cursor);
			let line_start = doc.content.line_to_char(line);
			let line_text: String = doc.content.line(line).chars().take(max_spaces).collect();
			let spaces = line_text
				.chars()
				.take_while(|c| *c == ' ')
				.count()
				.min(max_spaces);
			(line_start, spaces, buffer.cursor)
		};

		if spaces == 0 {
			return;
		}

		if !self.guard_readonly() {
			return;
		}

		self.save_undo_state();
		self.buffer_mut()
			.set_selection(Selection::single(line_start, line_start + spaces));

		let (tx, new_sel) = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
			let new_sel = tx.map_selection(&buffer.selection);
			(tx, new_sel)
		};
		self.buffer_mut().finalize_selection(new_sel);
		self.apply_transaction(&tx);

		let delete_end = line_start + spaces;
		let new_cursor = if old_cursor > delete_end {
			old_cursor.saturating_sub(spaces)
		} else if old_cursor > line_start {
			line_start
		} else {
			old_cursor
		};
		self.buffer_mut().set_cursor(new_cursor);
	}
}
