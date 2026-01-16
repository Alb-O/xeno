//! Data-oriented edit operation executor.
//!
//! This module provides a single executor function that processes [`EditOp`]
//! records. All text editing operations are expressed as data and processed
//! uniformly by this executor.
//!
//! # Compile -> Commit Pattern
//!
//! The executor compiles EditOp into an EditPlan with resolved policies, then
//! executes it with proper undo recording at the start (not sprinkled in each
//! transform). This ensures:
//!
//! - Undo recording happens once per operation (not per transform step)
//! - View snapshots are captured before any changes
//! - Syntax updates use the lazy `MarkDirty` policy by default
//!
//! # Execution Phases
//!
//! 1. Compile: Resolve undo/syntax policies based on transform type
//! 2. Undo setup: Save view snapshots and document undo state if needed
//! 3. Pre-effects: Execute yank, extend selection, etc. (NOT SaveUndo - handled above)
//! 4. Selection modification: Adjust selection before transform
//! 5. Text transformation: Apply the actual edit
//! 6. Post-effects: Mode change, cursor adjustment

use xeno_primitives::range::{Direction as MoveDir, Range};
use xeno_primitives::{Selection, Transaction, UndoPolicy};
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
	/// This is the core executor that handles undo recording at the start
	/// (based on the plan's policy) rather than sprinkling it in each transform.
	pub fn execute_edit_plan(&mut self, plan: EditPlan) {
		// Save undo state at the start if needed (not inside each transform)
		let needs_undo = matches!(
			plan.undo_policy,
			UndoPolicy::Record | UndoPolicy::Boundary | UndoPolicy::MergeWithCurrentGroup
		);

		// Check readonly before any mutation
		if plan.op.modifies_text() && !self.guard_readonly() {
			return;
		}

		// Save undo state once at the start (captures view snapshots + doc state)
		if needs_undo && plan.op.modifies_text() {
			self.save_undo_state();
		}

		for pre in &plan.op.pre {
			self.apply_pre_effect(pre);
		}

		self.apply_selection_op(&plan.op.selection);

		let original_cursor = self.buffer().cursor;
		self.apply_text_transform_with_plan(&plan);

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
			PreEffect::SaveUndo => {
				self.save_undo_state();
			}
			PreEffect::ExtendForwardIfEmpty => {
				if self.buffer().selection.primary().is_empty() {
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
										MoveDir::Forward,
										1,
										true,
									)
								})
								.collect()
						})
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
			}

			SelectionOp::ToLineStart => {
				let (new_ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let ranges: Vec<_> = buffer
							.selection
							.ranges()
							.iter()
							.map(|r| movement::move_to_line_start(doc.content().slice(..), *r, false))
							.collect();
						(ranges, buffer.selection.primary_index())
					})
				};
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, primary_index));
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

			SelectionOp::SelectWordBefore => {
				let (ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let text = doc.content().slice(..);
						let mut ranges = Vec::new();
						let mut primary_index = 0usize;
						for (idx, range) in buffer.selection.ranges().iter().enumerate() {
							if range.head == 0 {
								continue;
							}
							if idx == buffer.selection.primary_index() {
								primary_index = ranges.len();
							}
							let word_start = movement::move_to_prev_word_start(
								text,
								*range,
								1,
								WordType::Word,
								false,
							);
							ranges.push(Range::new(word_start.head, range.head));
						}
						(ranges, primary_index)
					})
				};

				if !ranges.is_empty() {
					self.buffer_mut()
						.set_selection(Selection::from_vec(ranges, primary_index));
				}
			}

			SelectionOp::SelectWordAfter => {
				let (ranges, primary_index) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let text = doc.content().slice(..);
						let len = text.len_chars();
						let mut ranges = Vec::new();
						let mut primary_index = 0usize;
						for (idx, range) in buffer.selection.ranges().iter().enumerate() {
							if range.head >= len {
								continue;
							}
							if idx == buffer.selection.primary_index() {
								primary_index = ranges.len();
							}
							let word_end = movement::move_to_next_word_start(
								text,
								*range,
								1,
								WordType::Word,
								false,
							);
							ranges.push(Range::new(range.head, word_end.head));
						}
						(ranges, primary_index)
					})
				};

				if !ranges.is_empty() {
					self.buffer_mut()
						.set_selection(Selection::from_vec(ranges, primary_index));
				}
			}

			SelectionOp::SelectToNextLineStart => {
				let (selection, valid) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let primary = buffer.selection.primary();
						let line = doc.content().char_to_line(primary.head);
						let total_lines = doc.content().len_lines();
						if line + 1 < total_lines {
							let end_of_line = doc.content().line_to_char(line + 1) - 1;
							(Selection::single(end_of_line, end_of_line + 1), true)
						} else {
							(buffer.selection.clone(), false)
						}
					})
				};
				if valid {
					self.buffer_mut().set_selection(selection);
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
			}
		}
	}

	/// Applies a text transformation using the compiled plan.
	///
	/// Unlike `apply_text_transform`, this version does not call `save_undo_state()`
	/// or `guard_readonly()` for each transform - those are handled once at the
	/// start of `execute_edit_plan`.
	fn apply_text_transform_with_plan(&mut self, plan: &EditPlan) {
		match &plan.op.transform {
			TextTransform::None => {}

			TextTransform::Delete => {
				if self.buffer().selection.primary().is_empty() {
					return;
				}
				let (tx, new_sel) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let tx = Transaction::delete(doc.content().slice(..), &buffer.selection);
						let new_sel = tx.map_selection(&buffer.selection);
						(tx, new_sel)
					})
				};
				self.buffer_mut().finalize_selection(new_sel);
				self.apply_transaction(&tx);
			}

			TextTransform::Replace(text) => {
				let (tx, new_sel) = {
					let buffer = self.buffer();
					buffer.with_doc(|doc| {
						let tx = Transaction::delete(doc.content().slice(..), &buffer.selection);
						let new_sel = tx.map_selection(&buffer.selection);
						(tx, new_sel)
					})
				};
				self.buffer_mut().finalize_selection(new_sel);
				self.apply_transaction(&tx);
				self.insert_text_no_undo(text);
			}

			TextTransform::Insert(text) => {
				self.insert_text_no_undo(text);
			}

			TextTransform::InsertNewlineWithIndent => {
				self.insert_newline_with_indent_no_undo();
			}

			TextTransform::MapChars(kind) => {
				self.apply_char_mapping_no_undo(*kind);
			}

			TextTransform::ReplaceEachChar(ch) => {
				self.apply_replace_each_char_no_undo(*ch);
			}

			TextTransform::Undo => {
				self.undo();
			}

			TextTransform::Redo => {
				self.redo();
			}

			TextTransform::Deindent { max_spaces } => {
				self.apply_deindent_no_undo(*max_spaces);
			}
		}
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
				buffer.with_doc(|doc| {
					let tx = Transaction::delete(doc.content().slice(..), &buffer.selection);
					let new_sel = tx.map_selection(&buffer.selection);
					(tx, new_sel)
				})
			};
			self.buffer_mut().set_selection(new_sel);
			self.apply_transaction(&tx);
		}

		self.insert_text(text);
	}

	/// Character mapping without undo recording (called from execute_edit_plan).
	fn apply_char_mapping_no_undo(&mut self, kind: CharMapKind) {
		let primary = self.buffer().selection.primary();
		let from = primary.from();
		let to = primary.to();
		if from >= to {
			return;
		}

		let text: String = {
			let buffer = self.buffer();
			buffer.with_doc(|doc| {
				doc.content()
					.slice(from..to)
					.chars()
					.flat_map(|c| kind.apply(c))
					.collect()
			})
		};
		let new_len = text.chars().count();

		let (tx, new_sel) = {
			let buffer = self.buffer();
			buffer.with_doc(|doc| {
				let tx = Transaction::delete(doc.content().slice(..), &buffer.selection);
				let new_sel = tx.map_selection(&buffer.selection);
				(tx, new_sel)
			})
		};
		self.buffer_mut().finalize_selection(new_sel);
		self.apply_transaction(&tx);

		let tx = {
			let buffer = self.buffer();
			buffer.with_doc(|doc| {
				Transaction::insert(doc.content().slice(..), &buffer.selection, text)
			})
		};
		self.apply_transaction(&tx);

		let new_cursor = self.buffer().selection.primary().head + new_len;
		self.buffer_mut()
			.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
	}

	/// Replace each char without undo recording (called from execute_edit_plan).
	fn apply_replace_each_char_no_undo(&mut self, ch: char) {
		let primary = self.buffer().selection.primary();
		let from = primary.from();
		let to = primary.to();

		if from < to {
			let len = to - from;
			let replacement: String = std::iter::repeat_n(ch, len).collect();

			let (tx, new_sel) = {
				let buffer = self.buffer();
				buffer.with_doc(|doc| {
					let tx = Transaction::delete(doc.content().slice(..), &buffer.selection);
					let new_sel = tx.map_selection(&buffer.selection);
					(tx, new_sel)
				})
			};
			self.buffer_mut().finalize_selection(new_sel);
			self.apply_transaction(&tx);

			let tx = {
				let buffer = self.buffer();
				buffer.with_doc(|doc| {
					Transaction::insert(doc.content().slice(..), &buffer.selection, replacement)
				})
			};
			self.apply_transaction(&tx);

			let new_cursor = self.buffer().selection.primary().head + len;
			self.buffer_mut()
				.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
		} else {
			self.buffer_mut()
				.set_selection(Selection::single(from, from + 1));

			let (tx, new_sel) = {
				let buffer = self.buffer();
				buffer.with_doc(|doc| {
					let tx = Transaction::delete(doc.content().slice(..), &buffer.selection);
					let new_sel = tx.map_selection(&buffer.selection);
					(tx, new_sel)
				})
			};
			self.buffer_mut().finalize_selection(new_sel);
			self.apply_transaction(&tx);

			let tx = {
				let buffer = self.buffer();
				buffer.with_doc(|doc| {
					Transaction::insert(doc.content().slice(..), &buffer.selection, ch.to_string())
				})
			};
			self.apply_transaction(&tx);

			let new_cursor = self.buffer().selection.primary().head + 1;
			self.buffer_mut()
				.set_cursor_and_selection(new_cursor, Selection::point(new_cursor));
		}
	}

	/// Deindent without undo recording (called from execute_edit_plan).
	fn apply_deindent_no_undo(&mut self, max_spaces: usize) {
		let (line_start, spaces, old_cursor) = {
			let buffer = self.buffer();
			buffer.with_doc(|doc| {
				let line = doc.content().char_to_line(buffer.cursor);
				let line_start = doc.content().line_to_char(line);
				let line_text: String =
					doc.content().line(line).chars().take(max_spaces).collect();
				let spaces = line_text
					.chars()
					.take_while(|c| *c == ' ')
					.count()
					.min(max_spaces);
				(line_start, spaces, buffer.cursor)
			})
		};

		if spaces == 0 {
			return;
		}

		self.buffer_mut()
			.set_selection(Selection::single(line_start, line_start + spaces));

		let (tx, new_sel) = {
			let buffer = self.buffer();
			buffer.with_doc(|doc| {
				let tx = Transaction::delete(doc.content().slice(..), &buffer.selection);
				let new_sel = tx.map_selection(&buffer.selection);
				(tx, new_sel)
			})
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

	/// Inserts text without undo recording (called from execute_edit_plan).
	fn insert_text_no_undo(&mut self, text: &str) {
		let buffer_id = self.focused_view();

		let (tx, new_selection) = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			buffer.prepare_insert(text)
		};

		self.apply_transaction_with_selection_inner(buffer_id, &tx, Some(new_selection));
	}

	/// Inserts newline with indent without undo recording (called from execute_edit_plan).
	fn insert_newline_with_indent_no_undo(&mut self) {
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
		self.insert_text_no_undo(&text);
	}

	/// Internal transaction application helper that doesn't do readonly checks.
	fn apply_transaction_with_selection_inner(
		&mut self,
		buffer_id: crate::buffer::BufferId,
		tx: &Transaction,
		new_selection: Option<Selection>,
	) -> bool {
		#[cfg(feature = "lsp")]
		let encoding = {
			let buffer = self
				.buffers
				.get_buffer(buffer_id)
				.expect("focused buffer must exist");
			self.lsp.incremental_encoding_for_buffer(buffer)
		};

		#[cfg(feature = "lsp")]
		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = if let Some(encoding) = encoding {
				buffer.apply_edit_with_lsp(tx, &self.config.language_loader, encoding)
			} else {
				buffer.apply_transaction_with_syntax(tx, &self.config.language_loader)
			};
			if applied && let Some(selection) = new_selection {
				buffer.finalize_selection(selection);
			}
			applied
		};

		#[cfg(not(feature = "lsp"))]
		let applied = {
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("focused buffer must exist");
			let applied = buffer.apply_transaction_with_syntax(tx, &self.config.language_loader);
			if applied && let Some(selection) = new_selection {
				buffer.finalize_selection(selection);
			}
			applied
		};

		if applied {
			self.sync_sibling_selections(tx);
			self.frame.dirty_buffers.insert(buffer_id);
		}

		applied
	}
}
