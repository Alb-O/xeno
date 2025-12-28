mod wrapping;

use std::time::{Duration, SystemTime};

use evildoer_manifest::{SplitAttrs, SplitBuffer, SplitColor};
use evildoer_tui::layout::{Constraint, Direction, Layout, Rect};
use evildoer_tui::style::{Color, Modifier, Style};
use evildoer_tui::text::{Line, Span};
use evildoer_tui::widgets::{Block, Clear, Paragraph};

use super::buffer::{BufferRenderContext, ensure_buffer_cursor_visible};
use crate::Editor;
use crate::buffer::{BufferView, SplitDirection};

fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
	match color {
		Color::Rgb(r, g, b) => Some((r, g, b)),
		_ => None,
	}
}

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
	/// - `frame`: The evildoer_tui frame to render into
	pub fn render(&mut self, frame: &mut evildoer_tui::Frame) {
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
		self.notifications
			.render(notifications_area, frame.buffer_mut());
	}

	/// Renders all views in the layout with proper split handling.
	///
	/// This handles both text buffers and terminals in the same layout.
	fn render_split_buffers(
		&mut self,
		frame: &mut evildoer_tui::Frame,
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
						let size = evildoer_manifest::SplitSize::new(area.width, area.height);
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
			show_indent_guides: true,
		};

		// Render each view
		for (view, area) in &view_areas {
			let is_focused = *view == focused_view;

			match view {
				BufferView::Text(buffer_id) => {
					if let Some(buffer) = self.get_buffer(*buffer_id) {
						let result = ctx.render_buffer(buffer, *area, use_block_cursor, is_focused);
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

		// Check if mouse has slowed down over a separator that was previously suppressed
		// This handles the case where mouse was moving fast, then stopped over a separator
		if self.layout.hovered_separator.is_none()
			&& self.layout.separator_under_mouse.is_some()
			&& !self.layout.is_mouse_fast()
		{
			let old_hover = self.layout.hovered_separator.take();
			self.layout.hovered_separator = self.layout.separator_under_mouse;
			if old_hover != self.layout.hovered_separator {
				self.layout
					.update_hover_animation(old_hover, self.layout.hovered_separator);
				self.needs_redraw = true;
			}
		}

		// Check for hovered separator
		let hovered_rect = self.layout.hovered_separator.map(|(_, rect)| rect);

		// Get the current rect of the separator being dragged (if any)
		let dragging_rect = self.layout.drag_state().and_then(|drag_state| {
			self.layout
				.separator_rect_at_path(doc_area, &drag_state.path)
				.map(|(_, rect)| rect)
		});

		// Get animation state for fading
		let anim_rect = self.layout.animation_rect();
		let anim_intensity = self.layout.animation_intensity();

		// Request redraw if animation is in progress (including debounce period)
		if self.layout.animation_needs_redraw() {
			self.needs_redraw = true;
		}

		// Define colors for lerping
		let normal_fg: Color = self.theme.colors.ui.gutter_fg.into();
		let hover_fg: Color = self.theme.colors.ui.cursor_fg.into();
		let hover_bg: Color = self.theme.colors.ui.selection_bg.into();
		let normal_bg: Color = self.theme.colors.ui.bg.into();
		// High contrast colors for active drag - use main text fg/bg for maximum visibility
		let drag_fg: Color = self.theme.colors.ui.bg.into();
		let drag_bg: Color = self.theme.colors.ui.fg.into();

		for (direction, _pos, sep_rect) in separators {
			// Highlight if hovered or being dragged
			let is_hovered = hovered_rect == Some(sep_rect);
			let is_dragging = dragging_rect == Some(sep_rect);
			let is_animating = anim_rect == Some(sep_rect);

			let sep_char = match direction {
				SplitDirection::Horizontal => "\u{2502}", // Vertical line │
				SplitDirection::Vertical => "\u{2500}",   // Horizontal line ─
			};

			// Calculate separator style with animation
			// Priority: dragging > animating > hovered > normal
			// Note: We check is_animating before is_hovered because we want the
			// smooth fade-in animation even when hovered (intensity goes 0.0 -> 1.0).
			// The animation handles both fade-in and fade-out correctly via intensity.
			let sep_style = if is_dragging {
				// High contrast when actively dragging
				Style::default().fg(drag_fg).bg(drag_bg)
			} else if is_animating {
				// Animating - lerp between normal and hover states
				// This handles both fade-in (hovering) and fade-out (leaving)
				use evildoer_tui::animation::Animatable;

				use crate::test_events::SeparatorAnimationEvent;

				let fg = normal_fg.lerp(&hover_fg, anim_intensity);
				let bg = normal_bg.lerp(&hover_bg, anim_intensity);

				if let (Some(fg_rgb), Some(bg_rgb)) = (color_to_rgb(fg), color_to_rgb(bg)) {
					SeparatorAnimationEvent::frame(anim_intensity, fg_rgb, bg_rgb);
				}

				Style::default().fg(fg).bg(bg)
			} else if is_hovered {
				// Fully hovered (animation complete or no animation)
				Style::default().fg(hover_fg).bg(hover_bg)
			} else {
				// Normal state
				Style::default().fg(normal_fg)
			};

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
		frame: &mut evildoer_tui::Frame,
		terminal: &crate::terminal::TerminalBuffer,
		area: Rect,
		is_focused: bool,
	) {
		let base_style = Style::default()
			.bg(self.theme.colors.popup.bg.into())
			.fg(self.theme.colors.popup.fg.into());

		frame.render_widget(Block::default().style(base_style), area);

		let mut cells_to_render = Vec::new();
		terminal.for_each_cell(|row, col, cell| {
			if row < area.height && col < area.width && !cell.wide_continuation {
				cells_to_render.push((row, col, cell.clone(), terminal.is_selected(row, col)));
			}
		});

		let buf = frame.buffer_mut();
		for (row, col, cell, selected) in cells_to_render {
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

			if cell.attrs.contains(SplitAttrs::INVERSE) != selected {
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
		}

		if is_focused && let Some(cursor) = terminal.cursor() {
			let x = area.x + cursor.col;
			let y = area.y + cursor.row;
			if x < area.x + area.width && y < area.y + area.height {
				frame.set_cursor_position(evildoer_tui::layout::Position { x, y });
			}
		}
	}
}

/// Converts a SplitColor to a evildoer_tui Color.
fn convert_split_color(color: SplitColor) -> Color {
	match color {
		SplitColor::Indexed(i) => Color::Indexed(i),
		SplitColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
	}
}
