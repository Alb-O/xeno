//! Editor-level navigation operations.
//!
//! Most navigation is delegated to Buffer. This module provides
//! Editor-specific wrappers where needed.

use std::path::PathBuf;

use xeno_primitives::ScrollDirection;
use xeno_primitives::range::Direction as MoveDir;
use xeno_primitives::selection::Selection;
use xeno_registry::options::keys;

use super::Editor;
use crate::buffer::BufferId;

/// Target location for navigation.
#[derive(Debug, Clone)]
pub struct Location {
	/// File path to navigate to.
	pub path: PathBuf,
	/// Line number (0-indexed).
	pub line: usize,
	/// Column within the line (0-indexed, in characters).
	pub column: usize,
}

impl Location {
	/// Creates a new location.
	pub fn new(path: impl Into<PathBuf>, line: usize, column: usize) -> Self {
		Self {
			path: path.into(),
			line,
			column,
		}
	}

	/// Creates a location from LSP Position (line and character are 0-indexed).
	#[cfg(feature = "lsp")]
	pub fn from_lsp(path: impl Into<PathBuf>, position: &xeno_lsp::lsp_types::Position) -> Self {
		Self {
			path: path.into(),
			line: position.line as usize,
			column: position.character as usize,
		}
	}
}

impl Editor {
	/// Returns the line number containing the cursor.
	pub fn cursor_line(&self) -> usize {
		self.buffer().cursor_line()
	}

	/// Returns the column of the cursor within its line.
	pub fn cursor_col(&self) -> usize {
		self.buffer().cursor_col()
	}

	/// Computes the gutter width based on total line count.
	pub fn gutter_width(&self) -> u16 {
		self.buffer().gutter_width()
	}

	/// Moves cursors vertically, accounting for line wrapping.
	///
	/// Resolves the `tab-width` option and delegates to Buffer.
	pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
		let tab_width = self.tab_width();
		self.buffer_mut()
			.move_visual_vertical(direction, count, extend, tab_width);
	}

	/// Handles mouse scroll events.
	///
	/// Resolves `scroll-lines` and `tab-width` options and delegates to Buffer.
	pub(crate) fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize) {
		let scroll_lines = (self.option(keys::SCROLL_LINES) as usize).max(1);
		let tab_width = self.tab_width();
		self.buffer_mut()
			.handle_mouse_scroll(direction, count * scroll_lines, tab_width);
	}

	/// Navigates to a specific location (file, line, column).
	///
	/// If the file is already open in a buffer, switches to it. Otherwise,
	/// opens the file in a new buffer. Then positions the cursor at the
	/// specified line and column.
	///
	/// # Arguments
	///
	/// * `location` - The target location (file path, line, column)
	///
	/// # Returns
	///
	/// The buffer ID of the target buffer, or an error if the file couldn't
	/// be opened.
	pub async fn goto_location(&mut self, location: &Location) -> anyhow::Result<BufferId> {
		// Check if we already have this file open
		let buffer_id = if let Some(id) = self.buffers.find_by_path(&location.path) {
			// Switch to existing buffer
			self.focus_buffer(id);
			id
		} else {
			// Open the file
			let id = self.open_file(location.path.clone()).await?;
			self.focus_buffer(id);
			id
		};

		// Position cursor at the target location
		self.goto_line_col(location.line, location.column);

		Ok(buffer_id)
	}

	/// Moves cursor to a specific line and column.
	///
	/// Line and column are 0-indexed. If the line doesn't exist, goes to the
	/// last line. If the column is past the end of the line, goes to the end
	/// of the line.
	pub fn goto_line_col(&mut self, line: usize, column: usize) {
		let buffer = self.buffer_mut();
		let target_pos = buffer.with_doc(|doc| {
			let content = doc.content();

			// Clamp line to valid range
			let total_lines = content.len_lines();
			let target_line = line.min(total_lines.saturating_sub(1));

			// Get line start position
			let line_start = content.line_to_char(target_line);

			// Get line length (excluding newline)
			let line_slice = content.line(target_line);
			let line_len = line_slice.len_chars();
			let line_content_len = if line_len > 0 && line_slice.char(line_len - 1) == '\n' {
				line_len - 1
			} else {
				line_len
			};

			// Clamp column to line length
			let target_col = column.min(line_content_len);
			line_start + target_col
		});

		// Set cursor and selection
		buffer.set_cursor(target_pos);
		buffer.set_selection(Selection::point(target_pos));
	}
}
