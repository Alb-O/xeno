use tome_base::range::{Direction as MoveDir, Range};
use tome_base::{Selection, Transaction};
use tome_manifest::{EditAction, Mode, ScrollAmount, ScrollDir, VisualDirection};
use tome_stdlib::movement;

use super::Editor;

impl Editor {
	pub(crate) fn do_execute_edit_action(&mut self, action: EditAction, _extend: bool) {
		match action {
			EditAction::Delete { yank } => {
				if yank {
					self.yank_selection();
				}
				if self.buffer().selection.primary().is_empty() {
					let buffer = self.buffer_mut();
					let slice = buffer.doc.slice(..);
					buffer.selection.transform_mut(|r| {
						*r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, true);
					});
				}
				if !self.buffer().selection.primary().is_empty() {
					self.save_undo_state();
					let tx =
						Transaction::delete(self.buffer().doc.slice(..), &self.buffer().selection);
					self.buffer_mut().selection = tx.map_selection(&self.buffer().selection);
					self.apply_transaction(&tx);
				}
			}
			EditAction::Change { yank } => {
				if yank {
					self.yank_selection();
				}
				if !self.buffer().selection.primary().is_empty() {
					self.save_undo_state();
					let tx =
						Transaction::delete(self.buffer().doc.slice(..), &self.buffer().selection);
					self.buffer_mut().selection = tx.map_selection(&self.buffer().selection);
					self.apply_transaction(&tx);
				}
				self.buffer_mut().input.set_mode(Mode::Insert);
			}
			EditAction::Yank => {
				self.yank_selection();
			}
			EditAction::Paste { before } => {
				if before {
					self.paste_before();
				} else {
					self.paste_after();
				}
			}
			EditAction::PasteAll { before } => {
				if before {
					self.paste_before();
				} else {
					self.paste_after();
				}
			}
			EditAction::ReplaceWithChar { ch } => {
				let primary = self.buffer().selection.primary();
				let from = primary.min();
				let to = primary.max();
				if from < to {
					self.save_undo_state();
					let len = to - from;
					let replacement = std::iter::repeat_n(ch, len).collect::<String>();
					let tx =
						Transaction::delete(self.buffer().doc.slice(..), &self.buffer().selection);
					self.buffer_mut().selection = tx.map_selection(&self.buffer().selection);
					self.apply_transaction(&tx);
					let tx = Transaction::insert(
						self.buffer().doc.slice(..),
						&self.buffer().selection,
						replacement,
					);
					self.apply_transaction(&tx);
					let buffer = self.buffer_mut();
					buffer.cursor = buffer.selection.primary().head + len;
					buffer.selection = Selection::point(buffer.cursor);
				} else {
					self.save_undo_state();
					self.buffer_mut().selection = Selection::single(from, from + 1);
					let tx =
						Transaction::delete(self.buffer().doc.slice(..), &self.buffer().selection);
					self.buffer_mut().selection = tx.map_selection(&self.buffer().selection);
					self.apply_transaction(&tx);
					let tx = Transaction::insert(
						self.buffer().doc.slice(..),
						&self.buffer().selection,
						ch.to_string(),
					);
					self.apply_transaction(&tx);
					let buffer = self.buffer_mut();
					buffer.cursor = buffer.selection.primary().head + 1;
					buffer.selection = Selection::point(buffer.cursor);
				}
			}
			EditAction::Undo => {
				self.undo();
			}
			EditAction::Redo => {
				self.redo();
			}
			EditAction::Indent => {
				{
					let buffer = self.buffer_mut();
					let slice = buffer.doc.slice(..);
					buffer.selection.transform_mut(|r| {
						*r = movement::move_to_line_start(slice, *r, false);
					});
				}
				self.insert_text("    ");
			}
			EditAction::Deindent => {
				let line = self.buffer().doc.char_to_line(self.buffer().cursor);
				let line_start = self.buffer().doc.line_to_char(line);
				let line_text: String = self.buffer().doc.line(line).chars().take(4).collect();
				let spaces = line_text.chars().take_while(|c| *c == ' ').count().min(4);
				if spaces > 0 {
					self.save_undo_state();
					self.buffer_mut().selection =
						Selection::single(line_start, line_start + spaces);
					let tx =
						Transaction::delete(self.buffer().doc.slice(..), &self.buffer().selection);
					self.buffer_mut().selection = tx.map_selection(&self.buffer().selection);
					self.apply_transaction(&tx);
					self.buffer_mut().cursor = self.buffer().cursor.saturating_sub(spaces);
				}
			}
			EditAction::ToLowerCase => {
				self.apply_case_conversion(|c| Box::new(c.to_lowercase()));
			}
			EditAction::ToUpperCase => {
				self.apply_case_conversion(|c| Box::new(c.to_uppercase()));
			}
			EditAction::SwapCase => {
				self.apply_case_conversion(|c| {
					if c.is_uppercase() {
						Box::new(c.to_lowercase())
					} else {
						Box::new(c.to_uppercase())
					}
				});
			}
			EditAction::JoinLines => {
				let primary = self.buffer().selection.primary();
				let line = self.buffer().doc.char_to_line(primary.head);
				if line + 1 < self.buffer().doc.len_lines() {
					self.save_undo_state();
					let end_of_line = self.buffer().doc.line_to_char(line + 1) - 1;
					self.buffer_mut().selection = Selection::single(end_of_line, end_of_line + 1);
					let tx =
						Transaction::delete(self.buffer().doc.slice(..), &self.buffer().selection);
					self.buffer_mut().selection = tx.map_selection(&self.buffer().selection);
					self.apply_transaction(&tx);
					let tx = Transaction::insert(
						self.buffer().doc.slice(..),
						&self.buffer().selection,
						" ".to_string(),
					);
					self.apply_transaction(&tx);
					let buffer = self.buffer_mut();
					buffer.cursor = buffer.selection.primary().head + 1;
					buffer.selection = Selection::point(buffer.cursor);
				}
			}
			EditAction::DeleteBack => {
				// Delete backward across all cursors (skip any at buffer start).
				let mut ranges = Vec::new();
				let mut primary_index = 0usize;
				for (idx, range) in self.buffer().selection.ranges().iter().enumerate() {
					if range.head == 0 {
						continue;
					}
					if idx == self.buffer().selection.primary_index() {
						primary_index = ranges.len();
					}
					ranges.push(Range::new(range.head - 1, range.head));
				}

				if ranges.is_empty() {
					return;
				}

				if matches!(self.mode(), Mode::Insert) {
					self.save_insert_undo_state();
				} else {
					self.save_undo_state();
				}
				let deletion_selection = Selection::from_vec(ranges, primary_index);
				let tx = Transaction::delete(self.buffer().doc.slice(..), &deletion_selection);
				let mut new_selection = tx.map_selection(&deletion_selection);
				new_selection.transform_mut(|r| {
					let pos = r.min();
					r.anchor = pos;
					r.head = pos;
				});
				self.apply_transaction(&tx);

				let buffer = self.buffer_mut();
				buffer.selection = new_selection;
				buffer.cursor = buffer.selection.primary().head;
			}
			EditAction::OpenBelow => {
				{
					let buffer = self.buffer_mut();
					let slice = buffer.doc.slice(..);
					buffer.selection.transform_mut(|r| {
						*r = movement::move_to_line_end(slice, *r, false);
					});
				}
				self.insert_text("\n");
				self.buffer_mut().input.set_mode(Mode::Insert);
			}
			EditAction::OpenAbove => {
				{
					let buffer = self.buffer_mut();
					let slice = buffer.doc.slice(..);
					buffer.selection.transform_mut(|r| {
						*r = movement::move_to_line_start(slice, *r, false);
					});
				}
				self.insert_text("\n");
				{
					let buffer = self.buffer_mut();
					let doc_slice = buffer.doc.slice(..);
					buffer.selection.transform_mut(|r| {
						*r = movement::move_vertically(doc_slice, *r, MoveDir::Backward, 1, false);
					});
				}
				self.buffer_mut().input.set_mode(Mode::Insert);
			}
			EditAction::MoveVisual {
				direction,
				count,
				extend,
			} => {
				let dir = match direction {
					VisualDirection::Up => MoveDir::Backward,
					VisualDirection::Down => MoveDir::Forward,
				};
				self.move_visual_vertical(dir, count, extend);
			}
			EditAction::Scroll {
				direction,
				amount,
				extend: scroll_extend,
			} => {
				let count = match amount {
					ScrollAmount::Line(n) => n,
					ScrollAmount::HalfPage => 10,
					ScrollAmount::FullPage => 20,
				};
				let dir = match direction {
					ScrollDir::Up => MoveDir::Backward,
					ScrollDir::Down => MoveDir::Forward,
				};
				self.move_visual_vertical(dir, count, scroll_extend);
			}
			EditAction::AddLineBelow => {
				let current_pos = self.buffer().cursor;
				// Move cursor to line end, insert newline, then restore cursor
				let line = self.buffer().doc.char_to_line(current_pos);
				let line_end = if line + 1 < self.buffer().doc.len_lines() {
					self.buffer().doc.line_to_char(line + 1).saturating_sub(1)
				} else {
					self.buffer().doc.len_chars()
				};
				self.buffer_mut().cursor = line_end;
				self.insert_text("\n");
				let buffer = self.buffer_mut();
				buffer.cursor = current_pos;
				buffer.selection = Selection::point(current_pos);
			}
			EditAction::AddLineAbove => {
				let current_pos = self.buffer().cursor;
				let line = self.buffer().doc.char_to_line(current_pos);
				let line_start = self.buffer().doc.line_to_char(line);
				self.buffer_mut().cursor = line_start;
				self.insert_text("\n");
				let buffer = self.buffer_mut();
				buffer.cursor = current_pos + 1;
				buffer.selection = Selection::point(current_pos + 1);
			}
		}
	}

	pub(crate) fn apply_case_conversion<F>(&mut self, char_mapper: F)
	where
		F: Fn(char) -> Box<dyn Iterator<Item = char>>,
	{
		let primary = self.buffer().selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from < to {
			self.save_undo_state();
			let text: String = self
				.buffer()
				.doc
				.slice(from..to)
				.chars()
				.flat_map(char_mapper)
				.collect();
			let new_len = text.chars().count();
			let tx = Transaction::delete(self.buffer().doc.slice(..), &self.buffer().selection);
			self.buffer_mut().selection = tx.map_selection(&self.buffer().selection);
			self.apply_transaction(&tx);
			let tx =
				Transaction::insert(self.buffer().doc.slice(..), &self.buffer().selection, text);
			self.apply_transaction(&tx);
			let buffer = self.buffer_mut();
			buffer.cursor = buffer.selection.primary().head + new_len;
			buffer.selection = Selection::point(buffer.cursor);
		}
	}
}
