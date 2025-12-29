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

	/// Renders all views across all layout layers.
	///
	/// Layer 0 is rendered first (base), then overlay layers on top.
	/// Each layer's views and separators are rendered together before moving to the next layer.
	fn render_split_buffers(
		&mut self,
		frame: &mut evildoer_tui::Frame,
		doc_area: Rect,
		use_block_cursor: bool,
	) {
		let focused_view = self.focused_view();

		let layer_count = self.layout.layer_count();
		let mut layer_data: Vec<(
			usize,
			Rect,
			Vec<(BufferView, Rect)>,
			Vec<(SplitDirection, u16, Rect)>,
		)> = Vec::new();

		for layer_idx in 0..layer_count {
			if self.layout.layer(layer_idx).is_some() {
				let layer_area = self.layout.layer_area(layer_idx, doc_area);
				let view_areas = self.layout.compute_view_areas_for_layer(layer_idx, layer_area);
				let separators = self.layout.separator_positions_for_layer(layer_idx, layer_area);
				layer_data.push((layer_idx, layer_area, view_areas, separators));
			}
		}

		for (_, _, view_areas, _) in &layer_data {
			for (view, area) in view_areas {
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
		}

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
		if self.layout.animation_needs_redraw() {
			self.needs_redraw = true;
		}

		use crate::editor::DragState;
		use crate::test_events::SeparatorAnimationEvent;
		use evildoer_tui::animation::Animatable;

		let hovered_rect = self.layout.hovered_separator.map(|(_, rect)| rect);
		let dragging_rect = self.layout.drag_state().and_then(|drag_state| match drag_state {
			DragState::Split { path, layer, .. } => self
				.layout
				.separator_rect_at_path(doc_area, path, *layer)
				.map(|(_, rect)| rect),
			DragState::LayerBoundary => self.layout.layer_boundary_separator(doc_area),
		});
		let anim_rect = self.layout.animation_rect();
		let anim_intensity = self.layout.animation_intensity();
		let layer_boundary = self.layout.layer_boundary_separator(doc_area);

		let normal_fg: Color = self.theme.colors.ui.gutter_fg.into();
		let hover_fg: Color = self.theme.colors.ui.cursor_fg.into();
		let hover_bg: Color = self.theme.colors.ui.selection_bg.into();
		let normal_bg: Color = self.theme.colors.ui.bg.into();
		let drag_fg: Color = self.theme.colors.ui.bg.into();
		let drag_bg: Color = self.theme.colors.ui.fg.into();

		let separator_style = |rect: Rect| -> Style {
			let is_hovered = hovered_rect == Some(rect);
			let is_dragging = dragging_rect == Some(rect);
			let is_animating = anim_rect == Some(rect);

			if is_dragging {
				Style::default().fg(drag_fg).bg(drag_bg)
			} else if is_animating {
				let fg = normal_fg.lerp(&hover_fg, anim_intensity);
				let bg = normal_bg.lerp(&hover_bg, anim_intensity);
				if let (Some(fg_rgb), Some(bg_rgb)) = (color_to_rgb(fg), color_to_rgb(bg)) {
					SeparatorAnimationEvent::frame(anim_intensity, fg_rgb, bg_rgb);
				}
				Style::default().fg(fg).bg(bg)
			} else if is_hovered {
				Style::default().fg(hover_fg).bg(hover_bg)
			} else {
				Style::default().fg(normal_fg)
			}
		};

		let ctx = BufferRenderContext {
			theme: self.theme,
			language_loader: &self.language_loader,
			style_overlays: &self.style_overlays,
			show_indent_guides: true,
		};

		for (_, _, view_areas, separators) in &layer_data {
			for (view, area) in view_areas {
				let is_focused = *view == focused_view;
				match view {
					BufferView::Text(buffer_id) => {
						if let Some(buffer) = self.get_buffer(*buffer_id) {
							let result =
								ctx.render_buffer(buffer, *area, use_block_cursor, is_focused);
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

			for (direction, _pos, sep_rect) in separators {
				let sep_style = separator_style(*sep_rect);
				let lines: Vec<Line> = match direction {
					SplitDirection::Horizontal => (0..sep_rect.height)
						.map(|_| Line::from(Span::styled("\u{2502}", sep_style)))
						.collect(),
					SplitDirection::Vertical => vec![Line::from(Span::styled(
						"\u{2500}".repeat(sep_rect.width as usize),
						sep_style,
					))],
				};
				frame.render_widget(Paragraph::new(lines), *sep_rect);
			}
		}

		if let Some(boundary_rect) = layer_boundary {
			let sep_style = separator_style(boundary_rect);
			let line = Line::from(Span::styled(
				"\u{2500}".repeat(boundary_rect.width as usize),
				sep_style,
			));
			frame.render_widget(Paragraph::new(vec![line]), boundary_rect);
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
