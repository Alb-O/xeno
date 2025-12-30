mod wrapping;

use std::time::{Duration, SystemTime};

use evildoer_manifest::{SplitAttrs, SplitColor};
use evildoer_tui::animation::Animatable;
use evildoer_tui::layout::{Constraint, Direction, Layout, Rect};
use evildoer_tui::style::{Color, Modifier, Style};
use evildoer_tui::text::{Line, Span};
use evildoer_tui::widgets::{Block, Clear, Paragraph};

use super::buffer::{BufferRenderContext, ensure_buffer_cursor_visible};
use crate::Editor;
use crate::buffer::{BufferView, SplitDirection};
use crate::test_events::SeparatorAnimationEvent;

fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
	match color {
		Color::Rgb(r, g, b) => Some((r, g, b)),
		_ => None,
	}
}

/// Precomputed separator colors and state for efficient style lookups.
struct SeparatorStyle {
	hovered_rect: Option<Rect>,
	dragging_rect: Option<Rect>,
	anim_rect: Option<Rect>,
	anim_intensity: f32,
	/// Base colors per visual priority level (index = priority).
	base_bg: [Color; 2],
	base_fg: [Color; 2],
	hover_fg: Color,
	hover_bg: Color,
	drag_fg: Color,
	drag_bg: Color,
}

impl SeparatorStyle {
	fn new(editor: &Editor, doc_area: Rect) -> Self {
		Self {
			hovered_rect: editor.layout.hovered_separator.map(|(_, rect)| rect),
			dragging_rect: editor
				.layout
				.drag_state()
				.and_then(|ds| editor.layout.separator_rect(doc_area, &ds.id)),
			anim_rect: editor.layout.animation_rect(),
			anim_intensity: editor.layout.animation_intensity(),
			base_bg: [editor.theme.colors.ui.bg, editor.theme.colors.popup.bg],
			base_fg: [
				editor.theme.colors.ui.gutter_fg,
				editor.theme.colors.popup.fg,
			],
			hover_fg: editor.theme.colors.ui.cursor_fg,
			hover_bg: editor.theme.colors.ui.selection_bg,
			drag_fg: editor.theme.colors.ui.bg,
			drag_bg: editor.theme.colors.ui.fg,
		}
	}

	fn for_rect(&self, rect: Rect, priority: u8) -> Style {
		let is_dragging = self.dragging_rect == Some(rect);
		let is_animating = self.anim_rect == Some(rect);
		let is_hovered = self.hovered_rect == Some(rect);

		let idx = (priority as usize).min(self.base_bg.len() - 1);
		let normal_fg = self.base_fg[idx];
		let normal_bg = self.base_bg[idx];

		if is_dragging {
			Style::default().fg(self.drag_fg).bg(self.drag_bg)
		} else if is_animating {
			let fg = normal_fg.lerp(&self.hover_fg, self.anim_intensity);
			let bg = normal_bg.lerp(&self.hover_bg, self.anim_intensity);
			if let (Some(fg_rgb), Some(bg_rgb)) = (color_to_rgb(fg), color_to_rgb(bg)) {
				SeparatorAnimationEvent::frame(self.anim_intensity, fg_rgb, bg_rgb);
			}
			Style::default().fg(fg).bg(bg)
		} else if is_hovered {
			Style::default().fg(self.hover_fg).bg(self.hover_bg)
		} else {
			Style::default().fg(normal_fg).bg(normal_bg)
		}
	}
}

/// Returns the box-drawing junction glyph for the given connectivity.
///
/// Connectivity is encoded as a 4-bit mask: up (0x1), down (0x2), left (0x4), right (0x8).
fn junction_glyph(connectivity: u8) -> char {
	match connectivity {
		0b1111 => '┼',
		0b1011 => '├',
		0b0111 => '┤',
		0b1110 => '┬',
		0b1101 => '┴',
		0b0011 => '│',
		0b1100 => '─',
		_ => '┼',
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

		let bg_block = Block::default().style(Style::default().bg(self.theme.colors.ui.bg));
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

		let status_bg = Block::default().style(Style::default().bg(self.theme.colors.popup.bg));
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
			Vec<(SplitDirection, u8, Rect)>,
		)> = Vec::new();

		for layer_idx in 0..layer_count {
			if self.layout.layer(layer_idx).is_some() {
				let layer_area = self.layout.layer_area(layer_idx, doc_area);
				let view_areas = self
					.layout
					.compute_view_areas_for_layer(layer_idx, layer_area);
				let separators = self
					.layout
					.separator_positions_for_layer(layer_idx, layer_area);
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
					BufferView::Panel(panel_id) => {
						self.resize_panel(*panel_id, *area);
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

		let layer_boundary = self.layout.layer_boundary_separator(doc_area);
		let sep_style = SeparatorStyle::new(self, doc_area);

		let ctx = BufferRenderContext {
			theme: self.theme,
			language_loader: &self.language_loader,
			style_overlays: &self.style_overlays,
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
					BufferView::Panel(panel_id) => {
						self.render_panel(frame, *panel_id, *area, is_focused);
					}
				}
			}

			for (direction, priority, sep_rect) in separators {
				let style = sep_style.for_rect(*sep_rect, *priority);
				let lines: Vec<Line> = match direction {
					SplitDirection::Horizontal => (0..sep_rect.height)
						.map(|_| Line::from(Span::styled("\u{2502}", style)))
						.collect(),
					SplitDirection::Vertical => vec![Line::from(Span::styled(
						"\u{2500}".repeat(sep_rect.width as usize),
						style,
					))],
				};
				frame.render_widget(Paragraph::new(lines), *sep_rect);
			}

			self.render_separator_junctions(frame, separators, &sep_style);
		}

		if let Some(boundary_rect) = layer_boundary {
			let boundary_priority = self.layout.layer_boundary_priority();
			let style = sep_style.for_rect(boundary_rect, boundary_priority);
			let line = Line::from(Span::styled(
				"\u{2500}".repeat(boundary_rect.width as usize),
				style,
			));
			frame.render_widget(Paragraph::new(vec![line]), boundary_rect);

			self.render_layer_boundary_junctions(
				frame,
				boundary_rect,
				boundary_priority,
				&layer_data,
				&sep_style,
			);
		}
	}

	/// Renders junction glyphs where separators intersect within a layer.
	fn render_separator_junctions(
		&self,
		frame: &mut evildoer_tui::Frame,
		separators: &[(SplitDirection, u8, Rect)],
		sep_style: &SeparatorStyle,
	) {
		use std::collections::HashMap;

		// SplitDirection::Vertical = stacked = horizontal line; Horizontal = side-by-side = vertical line
		let h_seps: Vec<_> = separators
			.iter()
			.filter(|(d, _, _)| *d == SplitDirection::Vertical)
			.collect();
		let v_seps: Vec<_> = separators
			.iter()
			.filter(|(d, _, _)| *d == SplitDirection::Horizontal)
			.collect();

		for (_, v_prio, v_rect) in &v_seps {
			let x = v_rect.x;
			let mut junctions: HashMap<u16, (bool, bool, bool, bool, u8)> = HashMap::new();

			for (_, h_prio, h_rect) in &h_seps {
				let y = h_rect.y;

				let at_left_edge = x + 1 == h_rect.x;
				let at_right_edge = x == h_rect.right();
				let within = x >= h_rect.x && x < h_rect.right();

				if !at_left_edge && !at_right_edge && !within {
					continue;
				}

				let touches_above = y >= v_rect.y && y < v_rect.bottom();
				let touches_below = y + 1 >= v_rect.y && y + 1 < v_rect.bottom();

				if touches_above || touches_below || within {
					let entry = junctions
						.entry(y)
						.or_insert((false, false, false, false, 0));
					if within {
						entry.0 |= y > v_rect.y;
						entry.1 |= y < v_rect.bottom().saturating_sub(1);
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else {
						entry.0 |= touches_above;
						entry.1 |= touches_below;
						entry.2 |= at_right_edge;
						entry.3 |= at_left_edge;
					}
					entry.4 = entry.4.max(*h_prio);
				}
			}

			let buf = frame.buffer_mut();
			for (y, (has_up, has_down, has_left, has_right, h_prio)) in junctions {
				let connectivity = (has_up as u8)
					| ((has_down as u8) << 1)
					| ((has_left as u8) << 2)
					| ((has_right as u8) << 3);

				if connectivity == 0b0011 {
					continue;
				}

				let glyph = junction_glyph(connectivity);
				let priority = (*v_prio).max(h_prio);
				let style = sep_style.for_rect(*v_rect, priority);

				if let Some(cell) = buf.cell_mut((x, y)) {
					cell.set_char(glyph);
					cell.set_style(style);
				}
			}
		}
	}

	/// Renders junction glyphs where layer boundary meets vertical separators.
	fn render_layer_boundary_junctions(
		&self,
		frame: &mut evildoer_tui::Frame,
		boundary_rect: Rect,
		boundary_priority: u8,
		layer_data: &[(
			usize,
			Rect,
			Vec<(BufferView, Rect)>,
			Vec<(SplitDirection, u8, Rect)>,
		)],
		sep_style: &SeparatorStyle,
	) {
		use std::collections::HashMap;

		let buf = frame.buffer_mut();
		let y = boundary_rect.y;

		let mut junctions: HashMap<u16, (bool, bool, u8)> = HashMap::new();

		for (layer_idx, _, _, separators) in layer_data {
			for (direction, priority, sep_rect) in separators {
				if *direction != SplitDirection::Horizontal {
					continue;
				}

				let x = sep_rect.x;
				if x < boundary_rect.x || x >= boundary_rect.right() {
					continue;
				}

				let touches_above = *layer_idx == 0 && sep_rect.bottom() == y;
				let touches_below = *layer_idx == 1 && sep_rect.y == y + 1;

				if touches_above || touches_below {
					let entry = junctions.entry(x).or_insert((false, false, 0));
					entry.0 |= touches_above;
					entry.1 |= touches_below;
					entry.2 = entry.2.max(*priority);
				}
			}
		}

		for (x, (has_up, has_down, priority)) in junctions {
			let has_left = x > boundary_rect.x;
			let has_right = x < boundary_rect.right().saturating_sub(1);

			let connectivity = (has_up as u8)
				| ((has_down as u8) << 1)
				| ((has_left as u8) << 2)
				| ((has_right as u8) << 3);

			let glyph = junction_glyph(connectivity);
			let style = sep_style.for_rect(boundary_rect, boundary_priority.max(priority));

			if let Some(cell) = buf.cell_mut((x, y)) {
				cell.set_char(glyph);
				cell.set_style(style);
			}
		}
	}

	/// Resizes a panel by ID.
	fn resize_panel(&mut self, panel_id: evildoer_manifest::PanelId, area: Rect) {
		let size = evildoer_manifest::SplitSize::new(area.width, area.height);
		if let Some(panel) = self.panels.get_mut(panel_id) {
			panel.resize(size);
		}
	}

	/// Renders a panel by ID.
	fn render_panel(
		&self,
		frame: &mut evildoer_tui::Frame,
		panel_id: evildoer_manifest::PanelId,
		area: Rect,
		is_focused: bool,
	) {
		if let Some(panel) = self.panels.get(panel_id) {
			render_split_buffer(frame, panel, area, is_focused, &self.theme.colors.popup);
		}
	}
}

/// Renders any SplitBuffer into the given area.
fn render_split_buffer(
	frame: &mut evildoer_tui::Frame,
	buffer: &dyn evildoer_manifest::SplitBuffer,
	area: Rect,
	is_focused: bool,
	colors: &evildoer_manifest::PopupColors,
) {
	let base_style = Style::default().bg(colors.bg).fg(colors.fg);

	frame.render_widget(Block::default().style(base_style), area);

	let mut cells_to_render = Vec::new();
	buffer.for_each_cell(&mut |row, col, cell| {
		if row < area.height && col < area.width && !cell.wide_continuation {
			let selected = buffer.is_selected(row, col);
			cells_to_render.push((row, col, cell.clone(), selected));
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

	if is_focused && let Some(cursor) = buffer.cursor() {
		let x = area.x + cursor.col;
		let y = area.y + cursor.row;
		if x < area.x + area.width && y < area.y + area.height {
			frame.set_cursor_position(evildoer_tui::layout::Position { x, y });
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
