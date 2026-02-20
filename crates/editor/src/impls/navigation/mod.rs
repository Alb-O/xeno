//! Editor-level navigation operations.
//!
//! Most navigation is delegated to Buffer. This module provides
//! Editor-specific wrappers where needed.

use std::path::PathBuf;

use xeno_primitives::ScrollDirection;
use xeno_primitives::range::Direction as MoveDir;
use xeno_primitives::selection::Selection;
use xeno_registry::HookEventData;
use xeno_registry::hooks::{HookContext, emit as emit_hook, emit_sync_with as emit_hook_sync_with};
use xeno_registry::options::option_keys as keys;

use super::Editor;
use crate::buffer::ViewId;

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
		self.buffer_mut().move_visual_vertical(direction, count, extend, tab_width);
	}

	/// Handles mouse scroll events.
	///
	/// Resolves `scroll-lines` and `tab-width` options and delegates to Buffer.
	pub(crate) fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize) {
		let scroll_lines = (self.option(keys::SCROLL_LINES) as usize).max(1);
		let tab_width = self.tab_width();
		self.buffer_mut().handle_mouse_scroll(direction, count * scroll_lines, tab_width);
		self.state.core.frame.needs_redraw = true;
	}

	/// Navigates to a specific location (file, line, column).
	///
	/// Opens a file into the currently focused split and then positions the cursor.
	///
	/// The focused view ID remains stable. If the target file is already open in
	/// another split, this view is rebound to the existing document.
	///
	/// # Arguments
	///
	/// * `location` - The target location (file path, line, column)
	///
	/// # Returns
	///
	/// The focused view ID, or an error if the file couldn't be opened.
	pub async fn goto_location(&mut self, location: &Location) -> anyhow::Result<ViewId> {
		let focused_view = self.base_window().focused_buffer;
		let target_path = crate::paths::fast_abs(&location.path);

		let already_focused_path = self
			.state
			.core
			.buffers
			.get_buffer(focused_view)
			.and_then(|buffer| buffer.path())
			.map(|path| crate::paths::fast_abs(&path) == target_path)
			.unwrap_or(false);

		if !already_focused_path {
			if let Some(old) = self.state.core.editor.buffers.get_buffer(focused_view) {
				let scratch_path = PathBuf::from("[scratch]");
				let path = old.path().unwrap_or_else(|| scratch_path.clone());
				let file_type = old.file_type();
				emit_hook_sync_with(
					&HookContext::new(HookEventData::BufferClose {
						path: &path,
						file_type: file_type.as_deref(),
					}),
					&mut self.state.integration.work_scheduler,
				);
			}

			let existing_view = self.state.core.editor.buffers.buffer_ids().collect::<Vec<_>>().into_iter().find(|id| {
				self.state
					.core
					.buffers
					.get_buffer(*id)
					.and_then(|buffer| buffer.path())
					.is_some_and(|path| crate::paths::fast_abs(&path) == target_path)
			});

			let is_existing = existing_view.is_some();
			let replacement = if let Some(source_view) = existing_view {
				let source = self.state.core.editor.buffers.get_buffer(source_view).expect("existing source buffer must be present");
				source.clone_for_split(focused_view)
			} else {
				self.load_file_buffer_for_view(focused_view, target_path.clone()).await?
			};

			let replaced = self
				.state
				.core
				.buffers
				.replace_buffer(focused_view, replacement)
				.expect("focused buffer must exist");

			self.finalize_document_if_orphaned(replaced.document_id());

			if !is_existing
				&& let Some(buffer) = self.state.core.editor.buffers.get_buffer(focused_view)
				&& let Some(path) = buffer.path()
			{
				let text = buffer.with_doc(|doc| doc.content().clone());
				let file_type = buffer.file_type();
				emit_hook(&HookContext::new(HookEventData::BufferOpen {
					path: &path,
					text: text.slice(..),
					file_type: file_type.as_deref(),
				}))
				.await;
			}

			// Nu on_hook (buffer_open event) — fires for both new and existing buffers.
			let kind = if is_existing { "existing" } else { "disk" };
			self.enqueue_buffer_open_hook(&target_path, kind);

			#[cfg(feature = "lsp")]
			self.maybe_track_lsp_for_buffer(focused_view, false);

			self.state.core.focus_epoch.increment();
			self.state.core.frame.needs_redraw = true;
		}

		self.goto_line_col(location.line, location.column);
		self.state.core.frame.needs_redraw = true;

		Ok(focused_view)
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

#[cfg(test)]
mod tests;
