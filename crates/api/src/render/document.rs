mod cursor;
mod viewport;
mod wrapping;

use std::time::{Duration, SystemTime};

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::buffer_render::{BufferRenderContext, ensure_buffer_cursor_visible};
use super::types::RenderResult;
use crate::buffer::SplitDirection;
use crate::Editor;

impl Editor {
	/// Renders the complete editor frame.
	///
	/// This is the main rendering entry point that orchestrates all UI elements:
	/// - Document content with cursor and selections (including splits)
	/// - UI panels (if any)
	/// - Command/message line
	/// - Status line
	/// - Notifications
	///
	/// # Parameters
	/// - `frame`: The ratatui frame to render into
	pub fn render(&mut self, frame: &mut ratatui::Frame) {
		let now = SystemTime::now();
		let delta = now
			.duration_since(self.last_tick)
			.unwrap_or(Duration::from_millis(16));
		self.last_tick = now;
		self.notifications.tick(delta);

		// Update style overlays to reflect current cursor position.
		// This must happen at render time (not tick time) to handle
		// mouse clicks and other events that modify cursor after tick.
		self.update_style_overlays();

		let use_block_cursor = true;

		let area = frame.area();
		self.window_width = Some(area.width);
		self.window_height = Some(area.height);

		frame.render_widget(Clear, area);

		let bg_block = Block::default().style(Style::default().bg(self.theme.colors.ui.bg.into()));
		frame.render_widget(bg_block, area);

		let chunks = Layout::default()
			.direction(Direction::Vertical)
			.constraints([Constraint::Min(1), Constraint::Length(1)])
			.split(area);

		let mut ui = std::mem::take(&mut self.ui);
		let dock_layout = ui.compute_layout(chunks[0]);
		let doc_area = dock_layout.doc_area;

		let doc_focused = ui.focus.focused().is_editor();

		// Render all buffers in the layout
		self.render_split_buffers(frame, doc_area, use_block_cursor && doc_focused);

		if let Some(cursor_pos) = ui.render_panels(self, frame, &dock_layout, self.theme) {
			frame.set_cursor_position(cursor_pos);
		}
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;

		let status_bg =
			Block::default().style(Style::default().bg(self.theme.colors.popup.bg.into()));
		frame.render_widget(status_bg, chunks[1]);
		frame.render_widget(self.render_status_line(), chunks[1]);

		let mut notifications_area = doc_area;
		notifications_area.height = notifications_area.height.saturating_sub(1);
		notifications_area.width = notifications_area.width.saturating_sub(1);
		self.notifications.render(frame, notifications_area);
	}

	/// Renders all buffers in the layout with proper split handling.
	fn render_split_buffers(
		&mut self,
		frame: &mut ratatui::Frame,
		doc_area: Rect,
		use_block_cursor: bool,
	) {
		let focused_id = self.focused_buffer_id();

		// Compute areas for all buffers in the layout
		let buffer_areas = self.layout.compute_areas(doc_area);

		// Ensure cursor is visible for each buffer in its area
		for (buffer_id, area) in &buffer_areas {
			if let Some(buffer) = self.get_buffer_mut(*buffer_id) {
				ensure_buffer_cursor_visible(buffer, *area);
			}
		}

		// Create render context
		let ctx = BufferRenderContext {
			theme: self.theme,
			language_loader: &self.language_loader,
			style_overlays: &self.style_overlays,
		};

		// Render each buffer
		for (buffer_id, area) in &buffer_areas {
			if let Some(buffer) = self.get_buffer(*buffer_id) {
				let is_focused = *buffer_id == focused_id;
				let result = ctx.render_buffer(buffer, *area, use_block_cursor && is_focused);
				frame.render_widget(result.widget, *area);
			}
		}

		// Render separators between splits
		let separators = self.layout.separator_positions(doc_area);
		for (direction, _pos, sep_rect) in separators {
			let sep_char = match direction {
				SplitDirection::Horizontal => "\u{2502}", // Vertical line │
				SplitDirection::Vertical => "\u{2500}",   // Horizontal line ─
			};
			let sep_style = Style::default().fg(self.theme.colors.ui.gutter_fg.into());

			// Build separator lines
			let lines: Vec<Line> = match direction {
				SplitDirection::Horizontal => {
					// Vertical separator - one character per row
					(0..sep_rect.height)
						.map(|_| Line::from(Span::styled(sep_char, sep_style)))
						.collect()
				}
				SplitDirection::Vertical => {
					// Horizontal separator - fill the width
					vec![Line::from(Span::styled(
						sep_char.repeat(sep_rect.width as usize),
						sep_style,
					))]
				}
			};

			frame.render_widget(Paragraph::new(lines), sep_rect);
		}
	}

	/// Renders the document with cursor tracking and visual effects.
	///
	/// This function handles the core document rendering logic including:
	/// - Line wrapping and viewport positioning
	/// - Cursor rendering (primary and secondary)
	/// - Selection highlighting
	/// - Gutter with line numbers
	/// - Cursor blinking in insert mode
	///
	/// # Parameters
	/// - `area`: The rectangular area to render the document into
	/// - `use_block_cursor`: Whether to render block-style cursors (normal mode)
	///   or rely on terminal cursor (insert mode)
	///
	/// # Returns
	/// A [`RenderResult`] containing the rendered paragraph widget.
	///
	/// # Note
	/// This method renders only the focused buffer. For split views, use
	/// `render_split_buffers` instead which handles multiple buffers.
	pub fn render_document_with_cursor(&self, area: Rect, use_block_cursor: bool) -> RenderResult {
		let ctx = BufferRenderContext {
			theme: self.theme,
			language_loader: &self.language_loader,
			style_overlays: &self.style_overlays,
		};
		ctx.render_buffer(self.buffer(), area, use_block_cursor)
	}
}
