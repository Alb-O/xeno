//! Document rendering logic for the editor.
//!
//! This module handles rendering of buffers in split views, including
//! separator styling and junction glyphs.

mod separator;
mod whichkey;

use std::time::{Duration, SystemTime};

use xeno_registry::options::keys;
use xeno_tui::layout::{Constraint, Direction, Layout, Rect};
use xeno_tui::style::Style;
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::{Block, Borders, Clear, Paragraph};

use self::separator::{SeparatorStyle, junction_glyph};
use super::buffer::{BufferRenderContext, GutterLayout, ensure_buffer_cursor_visible};
use crate::Editor;
use crate::buffer::{SplitDirection, ViewId};
use crate::impls::FocusTarget;
use crate::render::RenderCtx;
use crate::window::{GutterSelector, Window};

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
	/// - UI panels and docks
	/// - Floating windows and overlays
	/// - Command/message line and status line
	/// - Notifications
	///
	/// # Parameters
	/// - `frame`: The terminal frame to render into
	pub fn render(&mut self, frame: &mut xeno_tui::Frame) {
		let now = SystemTime::now();
		let delta = now
			.duration_since(self.state.frame.last_tick)
			.unwrap_or(Duration::from_millis(16));
		self.state.frame.last_tick = now;
		self.state.notifications.tick(delta);

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

		self.state.overlay_system.layers.render(self, frame);

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

	/// Renders all views and separators across all layout layers.
	///
	/// Orchestrates a two-pass rendering process:
	/// 1. Visibility pass: Ensures cursors are within visible viewports.
	/// 2. Render pass: Draws buffer content and gutters using the render cache.
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
					let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
					let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");
					let gutter = GutterSelector::Registry;
					let effective_gutter = if is_diff_file {
						BufferRenderContext::diff_gutter_selector(gutter)
					} else {
						gutter
					};

					let gutter_layout =
						GutterLayout::from_selector(effective_gutter, total_lines, area.width);
					let text_width = area.width.saturating_sub(gutter_layout.total_width) as usize;

					ensure_buffer_cursor_visible(
						buffer,
						*area,
						text_width,
						tab_width,
						scroll_margin,
					);
				}
			}
		}

		let sep_style = SeparatorStyle::new(ctx);

		let mut cache = std::mem::take(&mut self.state.render_cache);
		let language_loader = &self.state.config.language_loader;

		for (_, _, view_areas, _) in &layer_data {
			for (buffer_id, area) in view_areas {
				let is_focused = *buffer_id == focused_view;
				if let Some(buffer) = self.state.core.buffers.get_buffer(*buffer_id) {
					let tab_width = (buffer.option(keys::TAB_WIDTH, self) as usize).max(1);
					let cursorline = buffer.option(keys::CURSORLINE, self);

					let buffer_ctx = BufferRenderContext {
						theme: &ctx.theme,
						language_loader,
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
						&mut cache,
					);

					let gutter_area = Rect {
						width: result.gutter_width,
						..*area
					};
					let text_area = Rect {
						x: area.x + result.gutter_width,
						width: area.width.saturating_sub(result.gutter_width),
						..*area
					};

					frame.render_widget(Paragraph::new(result.gutter), gutter_area);
					frame.render_widget(Paragraph::new(result.text), text_area);
				}
			}
		}

		self.state.render_cache = cache;

		for (_, _, _, separators) in &layer_data {
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

		// Collect only IDs to avoid cloning windows; borrow windows in tight scopes
		let floating_ids: Vec<_> = self
			.state
			.windows
			.floating_windows()
			.map(|(id, _)| id)
			.collect();

		// First pass: ensure cursor visible (needs &mut self)
		for &window_id in &floating_ids {
			let (buffer_id, content_area, gutter_selector) = {
				let Some(window) = self.state.windows.get(window_id) else {
					continue;
				};
				let Window::Floating(window) = window else {
					continue;
				};
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
				(window.buffer, content_area, window.gutter)
			};

			let tab_width = self.tab_width_for(buffer_id);
			let scroll_margin = self.scroll_margin_for(buffer_id);
			if let Some(buffer) = self.get_buffer_mut(buffer_id) {
				let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
				let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");
				let effective_gutter = if is_diff_file {
					BufferRenderContext::diff_gutter_selector(gutter_selector)
				} else {
					gutter_selector
				};

				let gutter_layout =
					GutterLayout::from_selector(effective_gutter, total_lines, content_area.width);
				let text_width =
					content_area.width.saturating_sub(gutter_layout.total_width) as usize;

				ensure_buffer_cursor_visible(
					buffer,
					content_area,
					text_width,
					tab_width,
					scroll_margin,
				);
			}
		}

		// Second pass: render (borrows windows again)
		// Use mem::take to move cache out of self, allowing free use of &self in the loop
		let mut cache = std::mem::take(&mut self.state.render_cache);
		let language_loader = &self.state.config.language_loader;
		for &window_id in &floating_ids {
			let Some(window) = self.state.windows.get(window_id) else {
				continue;
			};
			let Window::Floating(window) = window else {
				continue;
			};
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

			if let Some(buffer) = self.state.core.buffers.get_buffer(window.buffer) {
				let is_focused = focused
					.map(|(win, buf)| win == window_id && buf == window.buffer)
					.unwrap_or(false);
				let tab_width = (buffer.option(keys::TAB_WIDTH, self) as usize).max(1);
				let cursorline = buffer.option(keys::CURSORLINE, self);

				let buffer_ctx = BufferRenderContext {
					theme: &ctx.theme,
					language_loader,
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
					&mut cache,
				);

				let gutter_area = Rect {
					width: result.gutter_width,
					..content_area
				};
				let text_area = Rect {
					x: content_area.x + result.gutter_width,
					width: content_area.width.saturating_sub(result.gutter_width),
					..content_area
				};

				frame.render_widget(Paragraph::new(result.gutter), gutter_area);
				frame.render_widget(Paragraph::new(result.text), text_area);
			}
		}

		// Put cache back
		self.state.render_cache = cache;
	}

	/// Renders junction glyphs where separators intersect within a layer.
	///
	/// Uses a raster-based approach: builds an occupancy map of separator cells,
	/// then derives junction connectivity from neighbor relationships. This is
	/// simpler and more correct than the previous O(n²) rectangle intersection
	/// logic with complex adjacency predicates.
	fn render_separator_junctions(
		&self,
		frame: &mut xeno_tui::Frame,
		separators: &[(SplitDirection, u8, Rect)],
		sep_style: &SeparatorStyle,
	) {
		use std::collections::HashMap;

		/// Occupancy state for a separator cell.
		#[derive(Debug, Clone, Copy, Default)]
		struct CellOcc {
			/// Has horizontal line (from Vertical split direction).
			has_h: bool,
			/// Has vertical line (from Horizontal split direction).
			has_v: bool,
			/// Maximum priority at this cell.
			prio: u8,
		}

		type OccMap = HashMap<(u16, u16), CellOcc>;

		let mut occ: OccMap = HashMap::new();

		// Rasterize all separators into occupancy map
		for (direction, prio, rect) in separators {
			match direction {
				// Horizontal split = side-by-side buffers = vertical separator line
				SplitDirection::Horizontal => {
					let x = rect.x;
					for y in rect.y..rect.bottom() {
						let cell = occ.entry((x, y)).or_default();
						cell.has_v = true;
						cell.prio = cell.prio.max(*prio);
					}
				}
				// Vertical split = stacked buffers = horizontal separator line
				SplitDirection::Vertical => {
					let y = rect.y;
					for x in rect.x..rect.right() {
						let cell = occ.entry((x, y)).or_default();
						cell.has_h = true;
						cell.prio = cell.prio.max(*prio);
					}
				}
			}
		}

		let buf = frame.buffer_mut();

		// Compute junctions from occupancy and neighbor relationships
		for (&(x, y), cell) in &occ {
			// Check neighbor occupancy for connectivity
			let has_up = occ.get(&(x, y.saturating_sub(1))).is_some_and(|c| c.has_v);
			let has_down = occ.get(&(x, y + 1)).is_some_and(|c| c.has_v);
			let has_left = occ.get(&(x.saturating_sub(1), y)).is_some_and(|c| c.has_h);
			let has_right = occ.get(&(x + 1, y)).is_some_and(|c| c.has_h);

			// Current cell contributes to its own direction
			let up = cell.has_v || has_up;
			let down = cell.has_v || has_down;
			let left = cell.has_h || has_left;
			let right = cell.has_h || has_right;

			// Only render junctions where lines actually meet or change
			let is_junction = (has_down || has_up || cell.has_v) && cell.has_h;

			if !is_junction {
				continue;
			}

			let connectivity =
				(up as u8) | ((down as u8) << 1) | ((left as u8) << 2) | ((right as u8) << 3);

			if connectivity == 0b0011 {
				continue; // Just a horizontal line segment
			}

			let glyph = junction_glyph(connectivity);
			let style = sep_style.for_junction(x, y, cell.prio);

			if let Some(buf_cell) = buf.cell_mut((x, y)) {
				buf_cell.set_char(glyph);
				buf_cell.set_style(style);
			}
		}
	}
}
