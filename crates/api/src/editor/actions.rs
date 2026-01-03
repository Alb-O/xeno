use tracing::debug;
use xeno_base::range::{Direction as MoveDir, Range};
use xeno_base::{Mode, Selection, Transaction};
use xeno_core::editor_ctx::ModeAccess;
use xeno_core::movement;
use xeno_registry::{EditAction, ScrollAmount, ScrollDir, VisualDirection};

use super::Editor;

impl Editor {
	/// Executes an edit action (delete, change, yank, etc.).
	pub(crate) fn do_execute_edit_action(&mut self, action: EditAction, _extend: bool) {
		debug!(edit = ?action, "Executing edit action");
		match action {
			EditAction::Delete { yank } => {
				if yank {
					self.yank_selection();
				}
				if self.buffer().selection.primary().is_empty() {
					// Extend selection forward by 1 char
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
					let buffer = self.buffer_mut();
					buffer.selection =
						Selection::from_vec(new_ranges, buffer.selection.primary_index());
				}
				if !self.buffer().selection.primary().is_empty() {
					self.save_undo_state();
					let (tx, new_sel) = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
						let new_sel = tx.map_selection(&buffer.selection);
						(tx, new_sel)
					};
					self.buffer_mut().selection = new_sel;
					self.apply_transaction(&tx);
				}
			}
			EditAction::Change { yank } => {
				if yank {
					self.yank_selection();
				}
				if !self.buffer().selection.primary().is_empty() {
					self.save_undo_state();
					let (tx, new_sel) = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
						let new_sel = tx.map_selection(&buffer.selection);
						(tx, new_sel)
					};
					self.buffer_mut().selection = new_sel;
					self.apply_transaction(&tx);
				}
				self.set_mode(Mode::Insert);
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
					let (tx, new_sel) = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
						let new_sel = tx.map_selection(&buffer.selection);
						(tx, new_sel)
					};
					self.buffer_mut().selection = new_sel;
					self.apply_transaction(&tx);
					let tx = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						Transaction::insert(doc.content.slice(..), &buffer.selection, replacement)
					};
					self.apply_transaction(&tx);
					let buffer = self.buffer_mut();
					buffer.cursor = buffer.selection.primary().head + len;
					buffer.selection = Selection::point(buffer.cursor);
				} else {
					self.save_undo_state();
					self.buffer_mut().selection = Selection::single(from, from + 1);
					let (tx, new_sel) = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
						let new_sel = tx.map_selection(&buffer.selection);
						(tx, new_sel)
					};
					self.buffer_mut().selection = new_sel;
					self.apply_transaction(&tx);
					let tx = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						Transaction::insert(
							doc.content.slice(..),
							&buffer.selection,
							ch.to_string(),
						)
					};
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
					let new_ranges: Vec<_> = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| movement::move_to_line_start(doc.content.slice(..), *r, false))
							.collect()
					};
					let buffer = self.buffer_mut();
					buffer.selection =
						Selection::from_vec(new_ranges, buffer.selection.primary_index());
				}
				self.insert_text("    ");
			}
			EditAction::Deindent => {
				let (line, line_start, spaces) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let line = doc.content.char_to_line(buffer.cursor);
					let line_start = doc.content.line_to_char(line);
					let line_text: String = doc.content.line(line).chars().take(4).collect();
					let spaces = line_text.chars().take_while(|c| *c == ' ').count().min(4);
					(line, line_start, spaces)
				};
				let _ = line;
				if spaces > 0 {
					self.save_undo_state();
					self.buffer_mut().selection =
						Selection::single(line_start, line_start + spaces);
					let (tx, new_sel) = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
						let new_sel = tx.map_selection(&buffer.selection);
						(tx, new_sel)
					};
					self.buffer_mut().selection = new_sel;
					self.apply_transaction(&tx);
					let cursor = self.buffer().cursor;
					self.buffer_mut().cursor = cursor.saturating_sub(spaces);
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
				let (line, total_lines, end_of_line) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let primary = buffer.selection.primary();
					let line = doc.content.char_to_line(primary.head);
					let total_lines = doc.content.len_lines();
					let end_of_line = if line + 1 < total_lines {
						doc.content.line_to_char(line + 1) - 1
					} else {
						0
					};
					(line, total_lines, end_of_line)
				};
				if line + 1 < total_lines {
					self.save_undo_state();
					self.buffer_mut().selection = Selection::single(end_of_line, end_of_line + 1);
					let (tx, new_sel) = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						let tx = Transaction::delete(doc.content.slice(..), &buffer.selection);
						let new_sel = tx.map_selection(&buffer.selection);
						(tx, new_sel)
					};
					self.buffer_mut().selection = new_sel;
					self.apply_transaction(&tx);
					let tx = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						Transaction::insert(
							doc.content.slice(..),
							&buffer.selection,
							" ".to_string(),
						)
					};
					self.apply_transaction(&tx);
					let buffer = self.buffer_mut();
					buffer.cursor = buffer.selection.primary().head + 1;
					buffer.selection = Selection::point(buffer.cursor);
				}
			}
			EditAction::DeleteBack => {
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

				if ranges.is_empty() {
					return;
				}

				if matches!(self.mode(), Mode::Insert) {
					self.save_insert_undo_state();
				} else {
					self.save_undo_state();
				}
				let deletion_selection = Selection::from_vec(ranges, primary_index);
				let (tx, new_selection) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let tx = Transaction::delete(doc.content.slice(..), &deletion_selection);
					let mut new_sel = tx.map_selection(&deletion_selection);
					new_sel.transform_mut(|r| {
						let pos = r.min();
						r.anchor = pos;
						r.head = pos;
					});
					(tx, new_sel)
				};
				self.apply_transaction(&tx);

				let buffer = self.buffer_mut();
				buffer.selection = new_selection;
				buffer.cursor = buffer.selection.primary().head;
			}
			EditAction::OpenBelow => {
				{
					let new_ranges: Vec<_> = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| movement::move_to_line_end(doc.content.slice(..), *r, false))
							.collect()
					};
					let buffer = self.buffer_mut();
					buffer.selection =
						Selection::from_vec(new_ranges, buffer.selection.primary_index());
				}
				self.insert_text("\n");
				self.set_mode(Mode::Insert);
			}
			EditAction::OpenAbove => {
				{
					let new_ranges: Vec<_> = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| movement::move_to_line_start(doc.content.slice(..), *r, false))
							.collect()
					};
					let buffer = self.buffer_mut();
					buffer.selection =
						Selection::from_vec(new_ranges, buffer.selection.primary_index());
				}
				self.insert_text("\n");
				{
					let new_ranges: Vec<_> = {
						let buffer = self.buffer();
						let doc = buffer.doc();
						buffer
							.selection
							.ranges()
							.iter()
							.map(|r| {
								movement::move_vertically(
									doc.content.slice(..),
									*r,
									MoveDir::Backward,
									1,
									false,
								)
							})
							.collect()
					};
					let buffer = self.buffer_mut();
					buffer.selection =
						Selection::from_vec(new_ranges, buffer.selection.primary_index());
				}
				self.set_mode(Mode::Insert);
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
				let (current_pos, line_end) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let current_pos = buffer.cursor;
					let line = doc.content.char_to_line(current_pos);
					let line_end = if line + 1 < doc.content.len_lines() {
						doc.content.line_to_char(line + 1).saturating_sub(1)
					} else {
						doc.content.len_chars()
					};
					(current_pos, line_end)
				};
				self.buffer_mut().cursor = line_end;
				self.insert_text("\n");
				let buffer = self.buffer_mut();
				buffer.cursor = current_pos;
				buffer.selection = Selection::point(current_pos);
			}
			EditAction::AddLineAbove => {
				let (current_pos, line_start) = {
					let buffer = self.buffer();
					let doc = buffer.doc();
					let current_pos = buffer.cursor;
					let line = doc.content.char_to_line(current_pos);
					let line_start = doc.content.line_to_char(line);
					(current_pos, line_start)
				};
				self.buffer_mut().cursor = line_start;
				self.insert_text("\n");
				let buffer = self.buffer_mut();
				buffer.cursor = current_pos + 1;
				buffer.selection = Selection::point(current_pos + 1);
			}
		}
	}

	/// Applies a character mapping function to the primary selection.
	pub(crate) fn apply_case_conversion<F>(&mut self, char_mapper: F)
	where
		F: Fn(char) -> Box<dyn Iterator<Item = char>>,
	{
		let primary = self.buffer().selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from < to {
			self.save_undo_state();
			let text: String = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				doc.content
					.slice(from..to)
					.chars()
					.flat_map(&char_mapper)
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
			self.buffer_mut().selection = new_sel;
			self.apply_transaction(&tx);
			let tx = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				Transaction::insert(doc.content.slice(..), &buffer.selection, text)
			};
			self.apply_transaction(&tx);
			let buffer = self.buffer_mut();
			buffer.cursor = buffer.selection.primary().head + new_len;
			buffer.selection = Selection::point(buffer.cursor);
		}
	}
}
