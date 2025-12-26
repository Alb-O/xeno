mod wrapping;

use std::time::{Duration, SystemTime};

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use tome_manifest::{SplitAttrs, SplitBuffer, SplitColor};

use super::buffer_render::{BufferRenderContext, ensure_buffer_cursor_visible};
use super::types::RenderResult;
use crate::buffer::{BufferView, SplitDirection};
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

	/// Renders all views in the layout with proper split handling.
	///
	/// This handles both text buffers and terminals in the same layout.
	fn render_split_buffers(
		&mut self,
		frame: &mut ratatui::Frame,
		doc_area: Rect,
		use_block_cursor: bool,
	) {
		let focused_view = self.focused_view();

		// Compute areas for all views in the layout
		let view_areas = self.layout.compute_view_areas(doc_area);

		// Ensure cursor is visible for each text buffer and resize terminals
		for (view, area) in &view_areas {
			match view {
				BufferView::Text(buffer_id) => {
					if let Some(buffer) = self.get_buffer_mut(*buffer_id) {
						ensure_buffer_cursor_visible(buffer, *area);
					}
				}
				BufferView::Terminal(terminal_id) => {
					if let Some(terminal) = self.get_terminal_mut(*terminal_id) {
						let size = tome_manifest::SplitSize::new(area.width, area.height);
						terminal.resize(size);
					}
				}
			}
		}

		// Create render context for text buffers
		let ctx = BufferRenderContext {
			theme: self.theme,
			language_loader: &self.language_loader,
			style_overlays: &self.style_overlays,
		};

		// Render each view
		for (view, area) in &view_areas {
			let is_focused = *view == focused_view;

			match view {
				BufferView::Text(buffer_id) => {
					if let Some(buffer) = self.get_buffer(*buffer_id) {
						let result =
							ctx.render_buffer(buffer, *area, use_block_cursor && is_focused);
						frame.render_widget(result.widget, *area);
					}
				}
				BufferView::Terminal(terminal_id) => {
					if let Some(terminal) = self.get_terminal(*terminal_id) {
						self.render_terminal(frame, terminal, *area, is_focused);
					}
				}
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

	/// Renders a terminal buffer into the given area.
	fn render_terminal(
		&self,
		frame: &mut ratatui::Frame,
		terminal: &crate::terminal_buffer::TerminalBuffer,
		area: Rect,
		_is_focused: bool,
	) {
		let base_style = Style::default()
			.bg(self.theme.colors.popup.bg.into())
			.fg(self.theme.colors.popup.fg.into());

		// Fill background
		frame.render_widget(Block::default().style(base_style), area);

		// Render terminal cells
		let buf = frame.buffer_mut();
		terminal.for_each_cell(|row, col, cell| {
			if row >= area.height || col >= area.width {
				return;
			}
			if cell.wide_continuation {
				return;
			}

			let x = area.x + col;
			let y = area.y + row;

			let mut style = base_style;

			if let Some(fg) = cell.fg {
				style = style.fg(convert_split_color(fg));
			}
			if let Some(bg) = cell.bg {
				style = style.bg(convert_split_color(bg));
			}

			let mut mods = Modifier::empty();
			if cell.attrs.contains(SplitAttrs::BOLD) {
				mods |= Modifier::BOLD;
			}
			if cell.attrs.contains(SplitAttrs::ITALIC) {
				mods |= Modifier::ITALIC;
			}
			if cell.attrs.contains(SplitAttrs::UNDERLINE) {
				mods |= Modifier::UNDERLINED;
			}
			style = style.add_modifier(mods);

			if cell.attrs.contains(SplitAttrs::INVERSE) {
				let fg = style.fg;
				let bg = style.bg;
				style = style.fg(bg.unwrap_or(Color::Reset));
				style = style.bg(fg.unwrap_or(Color::Reset));
			}

			let out = &mut buf[(x, y)];
			out.set_style(style);
			if cell.symbol.is_empty() {
				out.set_symbol(" ");
			} else {
				out.set_symbol(&cell.symbol);
			}
		});
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

/// Converts a SplitColor to a ratatui Color.
fn convert_split_color(color: SplitColor) -> Color {
	match color {
		SplitColor::Indexed(i) => Color::Indexed(i),
		SplitColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
	}
}
