use xeno_input::movement::{self, WordType};
use xeno_primitives::range::{Direction as MoveDir, Range};
use xeno_primitives::transaction::Change;
use xeno_primitives::{Selection, Transaction};
use xeno_registry::actions::edit_op::{
	CharMapKind, CursorAdjust, EditPlan, PostEffect, PreEffect, SelectionOp, TextTransform,
};

use super::super::Editor;

impl Editor {
	/// Applies a pre-effect before the main transformation.
	pub(super) fn apply_pre_effect(&mut self, effect: &PreEffect) {
		match effect {
			PreEffect::Yank => {
				self.yank_selection();
			}
		}
	}

	/// Applies a selection operation before text transformation.
	///
	/// These operations adjust the selection state (e.g., expanding to full lines,
	/// moving to line boundaries) before the primary [`TextTransform`] is executed.
	/// In Normal mode, resulting cursor positions are snapped to valid cell indices.
	///
	/// # Returns
	///
	/// Returns `false` if the operation produced no valid targets, signaling that the
	/// transform phase should be skipped.
	pub(super) fn apply_selection_op(&mut self, op: &SelectionOp) -> bool {
		let is_normal = self.mode() == xeno_primitives::Mode::Normal;
		match op {
			SelectionOp::None => true,

			SelectionOp::Extend { direction, count } => {
				let new_ranges: Vec<_> = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let text = doc.content().slice(..);
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								let mut new_range =
									movement::move_horizontally(text, *r, *direction, *count, true);
								if is_normal {
									new_range.head =
										xeno_primitives::rope::clamp_to_cell(new_range.head, text);
								}
								new_range
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
						let text = doc.content().slice(..);
						let ranges: Vec<_> = buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								let mut new_range = movement::move_to_line_start(text, *r, false);
								if is_normal {
									new_range.head =
										xeno_primitives::rope::clamp_to_cell(new_range.head, text);
									new_range.anchor = new_range.head;
								}
								new_range
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
						let text = doc.content().slice(..);
						let ranges: Vec<_> = buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								let mut new_range = movement::move_to_line_end(text, *r, false);
								if is_normal {
									new_range.head =
										xeno_primitives::rope::clamp_to_cell(new_range.head, text);
									new_range.anchor = new_range.head;
								}
								new_range
							})
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
								Range::from_exclusive(start, end)
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
				let new_sel = self
					.buffer()
					.selection
					.try_filter_transform(|r| (r.head > 0).then(|| Range::point(r.head - 1)));
				self.apply_selection_or_abort(new_sel)
			}

			SelectionOp::SelectCharAfter => {
				let new_sel = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let len = doc.content().len_chars();
						(len > 0).then(|| {
							buffer
								.selection
								.transform(|r| Range::point(r.head.min(len)))
						})
					})
				};
				self.apply_selection_or_abort(new_sel)
			}

			SelectionOp::SelectWordBefore => {
				let new_sel = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let text = doc.content().slice(..);
						buffer.selection.try_filter_transform(|r| {
							if r.head == 0 {
								return None;
							}
							let word_start = movement::move_to_prev_word_start(
								text,
								*r,
								1,
								WordType::Word,
								false,
							);
							(r.head > word_start.head)
								.then(|| Range::new(word_start.head, r.head - 1))
						})
					})
				};
				self.apply_selection_or_abort(new_sel)
			}

			SelectionOp::SelectWordAfter => {
				let new_sel = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let text = doc.content().slice(..);
						let len = text.len_chars();
						buffer.selection.try_filter_transform(|r| {
							if r.head >= len {
								return None;
							}
							let word_end = movement::move_to_next_word_start(
								text,
								*r,
								1,
								WordType::Word,
								false,
							);
							(word_end.head > r.head).then(|| Range::new(r.head, word_end.head - 1))
						})
					})
				};
				self.apply_selection_or_abort(new_sel)
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
							Some(Selection::point(end_of_line))
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
						let text = doc.content().slice(..);
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								let new_pos = if is_normal {
									xeno_primitives::rope::clamp_to_cell(r.head + 1, text)
								} else {
									(r.head + 1).min(text.len_chars())
								};
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

	/// Applies a new selection. Returns `false` if selection is `None`.
	pub(super) fn apply_selection_or_abort(&mut self, new_sel: Option<Selection>) -> bool {
		match new_sel {
			Some(sel) => {
				self.buffer_mut().set_selection(sel);
				true
			}
			None => false,
		}
	}

	/// Builds a transaction for the text transformation in the plan.
	///
	/// Returns `None` for transforms that don't produce a transaction (None, Undo, Redo)
	/// or when the transform has nothing to do (e.g., delete on empty selection).
	///
	/// Meta transforms (Undo/Redo) are executed directly since they don't produce
	/// a transaction that can be composed with apply_edit.
	pub(super) fn build_transform_transaction(
		&mut self,
		plan: &EditPlan,
	) -> Option<(Transaction, Selection)> {
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
	pub(super) fn apply_post_effect(&mut self, effect: &PostEffect, original_cursor: usize) {
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
	pub(super) fn build_delete_transaction(&self) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer();
		buffer.with_doc(|doc| {
			let len = doc.content().len_chars();
			let selection = if buffer.selection.primary().from() >= len && len > 0 {
				Selection::single(len - 1, len - 1)
			} else {
				buffer.selection.clone()
			};
			let tx = Transaction::delete(doc.content().slice(..), &selection);
			let new_sel = tx.map_selection(&selection);
			Some((tx, new_sel))
		})
	}

	/// Builds an insert transaction for the given text.
	pub(super) fn build_insert_transaction(
		&mut self,
		text: &str,
	) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer_mut();
		Some(buffer.prepare_insert(text))
	}

	/// Builds a newline+indent insert transaction.
	pub(super) fn build_newline_with_indent_transaction(
		&mut self,
	) -> Option<(Transaction, Selection)> {
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
	pub(super) fn build_replace_transaction(
		&mut self,
		replacement: &str,
	) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer();
		buffer.with_doc(|doc| {
			let replacement_str: String = replacement.into();
			let changes = buffer.selection.iter().map(|range| Change {
				start: range.from(),
				end: range.to(),
				replacement: Some(replacement_str.clone()),
			});
			let tx = Transaction::change(doc.content().slice(..), changes);
			let new_sel = tx.map_selection(&buffer.selection);
			Some((tx, new_sel))
		})
	}

	/// Builds a character mapping transaction (e.g., uppercase, lowercase, swap case).
	pub(super) fn build_char_mapping_transaction(
		&self,
		kind: CharMapKind,
	) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer();
		let primary = buffer.selection.primary();
		let from = primary.from();
		let to = primary.to();
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
				end: range.to(),
				replacement: Some(mapped.clone()),
			});
			let tx = Transaction::change(doc.content().slice(..), changes);
			let new_sel = tx.map_selection(&buffer.selection);
			Some((tx, new_sel))
		})
	}

	/// Builds a replace-each-char transaction (e.g., vim's `r` command).
	pub(super) fn build_replace_each_char_transaction(
		&self,
		ch: char,
	) -> Option<(Transaction, Selection)> {
		let buffer = self.buffer();
		let primary = buffer.selection.primary();
		let from = primary.from();
		let to = primary.to();

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
				end: range.to(),
				replacement: Some(replacement.clone()),
			});
			let tx = Transaction::change(doc.content().slice(..), changes);
			let new_sel = tx.map_selection(&selection);
			Some((tx, new_sel))
		})
	}

	/// Builds a deindent transaction.
	pub(super) fn build_deindent_transaction(
		&self,
		max_spaces: usize,
	) -> Option<(Transaction, Selection)> {
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
}
