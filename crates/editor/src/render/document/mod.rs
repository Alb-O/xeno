//! Document rendering logic for the editor.
//!
//! This module handles rendering of buffers in split views, including
//! separator styling and junction glyphs.

mod separator;
mod whichkey;

use std::time::{Duration, SystemTime};

use xeno_tui::layout::{Constraint, Direction, Layout, Rect};
use xeno_tui::style::Style;
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::{Block, Borders, Clear, Paragraph};

use self::separator::{SeparatorStyle, junction_glyph};
use super::buffer::{BufferRenderContext, ensure_buffer_cursor_visible};
use crate::Editor;
use crate::buffer::{SplitDirection, ViewId};
use crate::impls::FocusTarget;
use crate::render::RenderCtx;

/// Per-layer rendering data: (layer_index, layer_area, view_areas, separators).
type LayerRenderData = (
	usize,
	Rect,
	Vec<(ViewId, Rect)>,
	Vec<(SplitDirection, u8, Rect)>,
);

/// Clamps a rectangle to a bounding area, returning the intersection.
fn clamp_rect(rect: Rect, bounds: Rect) -> Option<Rect> {
	let x1 = rect.x.max(bounds.x);
	let y1 = rect.y.max(bounds.y);
	let x2 = rect.right().min(bounds.right());
	let y2 = rect.bottom().min(bounds.bottom());

	if x2 <= x1 || y2 <= y1 {
		return None;
	}

	Some(Rect {
		x: x1,
		y: y1,
		width: x2.saturating_sub(x1),
		height: y2.saturating_sub(y1),
	})
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
	/// - `frame`: The xeno_tui frame to render into
	pub fn render(&mut self, frame: &mut xeno_tui::Frame) {
		let now = SystemTime::now();
		let delta = now
			.duration_since(self.state.frame.last_tick)
			.unwrap_or(Duration::from_millis(16));
		self.state.frame.last_tick = now;
		self.state.notifications.tick(delta);

		// Update style overlays to reflect current cursor position.
		// This must happen at render time (not tick time) to handle
		// mouse clicks and other events that modify cursor after tick.
		self.update_style_overlays();

		// Poll background syntax parsing, installing results if ready.
		self.ensure_syntax_for_buffers();

		let use_block_cursor = true;

		let area = frame.area();
		self.state.viewport.width = Some(area.width);
		self.state.viewport.height = Some(area.height);

		frame.render_widget(Clear, area);

		let bg_block =
			Block::default().style(Style::default().bg(self.state.config.theme.colors.ui.bg));
		frame.render_widget(bg_block, area);

		let chunks = Layout::default()
			.direction(Direction::Vertical)
			.constraints([Constraint::Min(1), Constraint::Length(1)])
			.split(area);

		let main_area = chunks[0];
		let status_area = chunks[1];

		let mut ui = std::mem::take(&mut self.state.ui);
		let dock_layout = ui.compute_layout(main_area);
		let doc_area = dock_layout.doc_area;
		self.state.viewport.doc_area = Some(doc_area);

		if self.state.layout.hovered_separator.is_none()
			&& self.state.layout.separator_under_mouse.is_some()
			&& !self.state.layout.is_mouse_fast()
		{
			let old_hover = self.state.layout.hovered_separator.take();
			self.state.layout.hovered_separator = self.state.layout.separator_under_mouse;
			if old_hover != self.state.layout.hovered_separator {
				self.state
					.layout
					.update_hover_animation(old_hover, self.state.layout.hovered_separator);
				self.state.frame.needs_redraw = true;
			}
		}
		if self.state.layout.animation_needs_redraw() {
			self.state.frame.needs_redraw = true;
		}

		let ctx = self.render_ctx();
		let doc_focused = ui.focus.focused().is_editor();

		// Render all buffers in the layout
		self.render_split_buffers(frame, doc_area, use_block_cursor && doc_focused, &ctx);
		self.render_floating_windows(frame, use_block_cursor && doc_focused, &ctx);

		if let Some(cursor_pos) = ui.render_panels(self, frame, &dock_layout, &ctx.theme) {
			frame.set_cursor_position(cursor_pos);
		}
		if ui.take_wants_redraw() {
			self.state.frame.needs_redraw = true;
		}
		self.state.ui = ui;

		self.render_completion_popup(frame);

		let status_bg = Block::default().style(Style::default().bg(ctx.theme.colors.popup.bg));
		frame.render_widget(status_bg, status_area);
		frame.render_widget(self.render_status_line(), status_area);

		let mut notifications_area = doc_area;
		notifications_area.height = notifications_area.height.saturating_sub(1);
		notifications_area.width = notifications_area.width.saturating_sub(1);
		self.state
			.notifications
			.render(notifications_area, frame.buffer_mut());

		self.render_whichkey_hud(frame, doc_area, &ctx);
	}

	/// Renders all views across all layout layers.
	///
	/// Layer 0 is rendered first (base), then overlay layers on top.
	/// Each layer's views and separators are rendered together before moving to the next layer.
	fn render_split_buffers(
		&mut self,
		frame: &mut xeno_tui::Frame,
		doc_area: Rect,
		use_block_cursor: bool,
		ctx: &RenderCtx,
	) {
		let focused_view = self.focused_view();
		let base_layout = &self.base_window().layout;

		let layer_count = self.state.layout.layer_count();
		let mut layer_data: Vec<LayerRenderData> = Vec::new();

		for layer_idx in 0..layer_count {
			if self.state.layout.layer(base_layout, layer_idx).is_some() {
				let layer_area = self.state.layout.layer_area(layer_idx, doc_area);
				let view_areas = self.state.layout.compute_view_areas_for_layer(
					base_layout,
					layer_idx,
					layer_area,
				);
				let separators = self.state.layout.separator_positions_for_layer(
					base_layout,
					layer_idx,
					layer_area,
				);
				layer_data.push((layer_idx, layer_area, view_areas, separators));
			}
		}

		// During mouse drag (text_selection_origin is Some), disable scroll margin
		// to allow cursor to reach screen edges without triggering scrolloff.
		let mouse_drag_active = self.state.layout.text_selection_origin.is_some();
		for (_, _, view_areas, _) in &layer_data {
			for (buffer_id, area) in view_areas {
				let tab_width = self.tab_width_for(*buffer_id);
				let scroll_margin = if mouse_drag_active {
					0
				} else {
					self.scroll_margin_for(*buffer_id)
				};
				if let Some(buffer) = self.get_buffer_mut(*buffer_id) {
					ensure_buffer_cursor_visible(buffer, *area, tab_width, scroll_margin);
				}
			}
		}

		let sep_style = SeparatorStyle::new(ctx);

		for (_, _, view_areas, separators) in &layer_data {
			for (buffer_id, area) in view_areas {
				let is_focused = *buffer_id == focused_view;
				let tab_width = self.tab_width_for(*buffer_id);
				let cursorline = self.cursorline_for(*buffer_id);
				if let Some(buffer) = self.get_buffer(*buffer_id) {
					let buffer_ctx = BufferRenderContext {
						theme: &ctx.theme,
						language_loader: &self.state.config.language_loader,
						style_overlays: &ctx.style_overlays,
						diagnostics: ctx.lsp.diagnostics_for(*buffer_id),
						diagnostic_ranges: ctx.lsp.diagnostic_ranges_for(*buffer_id),
					};
					let result = buffer_ctx.render_buffer(
						buffer,
						*area,
						use_block_cursor,
						is_focused,
						tab_width,
						cursorline,
					);
					frame.render_widget(result.widget, *area);
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
	}

	/// Renders floating windows above the base layout.
	fn render_floating_windows(
		&mut self,
		frame: &mut xeno_tui::Frame,
		use_block_cursor: bool,
		ctx: &RenderCtx,
	) {
		let bounds = frame.area();
		let focused = match &self.state.focus {
			FocusTarget::Buffer { window, buffer } => Some((*window, *buffer)),
			_ => None,
		};

		let floating_windows: Vec<_> = self
			.state
			.windows
			.floating_windows()
			.map(|(id, window)| (id, window.clone()))
			.collect();
		for (_, window) in &floating_windows {
			let Some(rect) = clamp_rect(window.rect, bounds) else {
				continue;
			};
			let content_area = if window.style.border {
				Rect {
					x: rect.x.saturating_add(1),
					y: rect.y.saturating_add(1),
					width: rect.width.saturating_sub(2),
					height: rect.height.saturating_sub(2),
				}
			} else {
				rect
			};

			if content_area.width == 0 || content_area.height == 0 {
				continue;
			}

			let tab_width = self.tab_width_for(window.buffer);
			let scroll_margin = self.scroll_margin_for(window.buffer);
			if let Some(buffer) = self.get_buffer_mut(window.buffer) {
				ensure_buffer_cursor_visible(buffer, content_area, tab_width, scroll_margin);
			}
		}

		for (window_id, window) in floating_windows {
			let Some(rect) = clamp_rect(window.rect, bounds) else {
				continue;
			};

			if window.style.shadow {
				let shadow_rect = Rect {
					x: rect.x.saturating_add(1),
					y: rect.y.saturating_add(1),
					width: rect.width,
					height: rect.height,
				};
				if let Some(shadow) = clamp_rect(shadow_rect, bounds) {
					let shadow_block =
						Block::default().style(Style::default().bg(ctx.theme.colors.ui.bg));
					frame.render_widget(shadow_block, shadow);
				}
			}

			frame.render_widget(Clear, rect);

			let mut block = Block::default()
				.style(Style::default().bg(ctx.theme.colors.popup.bg))
				.padding(window.style.padding);
			if window.style.border {
				block = block
					.borders(Borders::ALL)
					.border_type(window.style.border_type)
					.border_style(Style::default().fg(ctx.theme.colors.popup.fg));
				if let Some(title) = &window.style.title {
					block = block.title(title.as_str());
				}
			}

			let content_area = block.inner(rect);

			frame.render_widget(block, rect);

			if content_area.width == 0 || content_area.height == 0 {
				continue;
			}

			if let Some(buffer) = self.get_buffer(window.buffer) {
				let is_focused = focused
					.map(|(win, buf)| win == window_id && buf == window.buffer)
					.unwrap_or(false);
				let tab_width = self.tab_width_for(window.buffer);
				let cursorline = self.cursorline_for(window.buffer);

				let buffer_ctx = BufferRenderContext {
					theme: &ctx.theme,
					language_loader: &self.state.config.language_loader,
					style_overlays: &ctx.style_overlays,
					diagnostics: ctx.lsp.diagnostics_for(window.buffer),
					diagnostic_ranges: ctx.lsp.diagnostic_ranges_for(window.buffer),
				};
				let result = buffer_ctx.render_buffer_with_gutter(
					buffer,
					content_area,
					use_block_cursor,
					is_focused,
					window.gutter,
					tab_width,
					cursorline,
				);
				frame.render_widget(result.widget, content_area);
			}
		}
	}

	/// Renders junction glyphs where separators intersect within a layer.
	fn render_separator_junctions(
		&self,
		frame: &mut xeno_tui::Frame,
		separators: &[(SplitDirection, u8, Rect)],
		sep_style: &SeparatorStyle,
	) {
		use std::collections::HashMap;

		type JunctionState = (bool, bool, bool, bool, u8);
		type JunctionMap = HashMap<(u16, u16), JunctionState>;

		// SplitDirection::Vertical = stacked = horizontal line; Horizontal = side-by-side = vertical line
		let h_seps: Vec<_> = separators
			.iter()
			.filter(|(d, _, _)| *d == SplitDirection::Vertical)
			.collect();
		let v_seps: Vec<_> = separators
			.iter()
			.filter(|(d, _, _)| *d == SplitDirection::Horizontal)
			.collect();

		// (x, y) -> (has_up, has_down, has_left, has_right, priority)
		let mut all_junctions: JunctionMap = HashMap::new();

		for (_, v_prio, v_rect) in &v_seps {
			let x = v_rect.x;

			for (_, h_prio, h_rect) in &h_seps {
				let y = h_rect.y;

				let at_left_edge = x + 1 == h_rect.x;
				let at_right_edge = x == h_rect.right();
				let x_overlaps = x >= h_rect.x && x < h_rect.right();

				let touches_above = y >= v_rect.y && y < v_rect.bottom();
				let adjacent_above = y == v_rect.bottom();
				let adjacent_below = y + 1 == v_rect.y;
				let touches_below = y + 1 >= v_rect.y && y + 1 < v_rect.bottom();
				let within = x_overlaps && touches_above;

				let dominated_above = x_overlaps && adjacent_above;
				let dominated_below = x_overlaps && adjacent_below;

				if !(at_left_edge
					|| at_right_edge
					|| within || dominated_above
					|| dominated_below
					|| (x_overlaps && touches_below))
				{
					continue;
				}

				if touches_above || touches_below || within || dominated_above || dominated_below {
					let entry = all_junctions
						.entry((x, y))
						.or_insert((false, false, false, false, 0));
					if within {
						entry.0 |= y > v_rect.y;
						entry.1 |= y < v_rect.bottom().saturating_sub(1);
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else if dominated_above {
						entry.0 = true;
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else if dominated_below {
						entry.1 = true;
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else if x_overlaps {
						entry.0 |= touches_above;
						entry.1 |= touches_below;
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else {
						entry.0 |= touches_above;
						entry.1 |= touches_below;
						entry.2 |= at_right_edge;
						entry.3 |= at_left_edge;
					}
					entry.4 = entry.4.max(*v_prio).max(*h_prio);
				}
			}
		}

		let buf = frame.buffer_mut();
		for ((x, y), (has_up, has_down, has_left, has_right, priority)) in all_junctions {
			let connectivity = (has_up as u8)
				| ((has_down as u8) << 1)
				| ((has_left as u8) << 2)
				| ((has_right as u8) << 3);

			if connectivity == 0b0011 {
				continue;
			}

			let glyph = junction_glyph(connectivity);
			let style = sep_style.for_junction(x, y, priority);

			if let Some(cell) = buf.cell_mut((x, y)) {
				cell.set_char(glyph);
				cell.set_style(style);
			}
		}
	}
}
