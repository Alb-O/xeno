//! Document rendering logic for the editor.
//!
//! This module handles rendering of buffers in split views, including
//! separator styling and junction glyphs.

mod separator;

use xeno_registry::options::keys;
use xeno_tui::layout::Rect;
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::Paragraph;

use self::separator::{SeparatorStyle, junction_glyph};
use super::buffer::{BufferRenderContext, GutterLayout, ensure_buffer_cursor_visible};
use crate::Editor;
use crate::buffer::{SplitDirection, ViewId};
use crate::layout::LayerId;
use crate::render::RenderCtx;
use crate::window::GutterSelector;

/// Per-layer rendering data: (layer_id, layer_area, view_areas, separators).
type LayerRenderData = (LayerId, Rect, Vec<(ViewId, Rect)>, Vec<(SplitDirection, u8, Rect)>);

impl Editor {
	/// Renders the complete editor frame.
	///
	/// This is the main rendering entry point that orchestrates all UI elements:
	/// - Document content with cursor and selections (including splits)
	/// - UI panels and docks
	/// - Overlay surfaces
	/// - Command/message line and status line
	/// - Notifications
	///
	/// # Parameters
	/// - `frame`: The terminal frame to render into
	pub fn render(&mut self, frame: &mut xeno_tui::Frame) {
		crate::ui::compositor::render_frame(self, frame);
	}

	/// Renders all views and separators across all layout layers.
	///
	/// Orchestrates a two-pass rendering process:
	/// 1. Visibility pass: Ensures cursors are within visible viewports.
	/// 2. Render pass: Draws buffer content and gutters using the render cache.
	pub fn render_split_buffers(&mut self, frame: &mut xeno_tui::Frame, doc_area: Rect, use_block_cursor: bool, ctx: &RenderCtx) {
		let focused_view = self.focused_view();
		let base_layout = &self.base_window().layout;

		let layer_count = self.state.layout.layer_count();
		let mut layer_data: Vec<LayerRenderData> = Vec::new();

		// Base layer (index 0)
		{
			let layer_id = LayerId::BASE;
			let layer_area = self.state.layout.layer_area(layer_id, doc_area.into());
			let view_areas = self
				.state
				.layout
				.compute_view_areas_for_layer(base_layout, layer_id, layer_area)
				.into_iter()
				.map(|(view_id, rect)| (view_id, rect.into()))
				.collect();
			let separators = self
				.state
				.layout
				.separator_positions_for_layer(base_layout, layer_id, layer_area)
				.into_iter()
				.map(|(direction, priority, rect)| (direction, priority, rect.into()))
				.collect();
			layer_data.push((layer_id, layer_area.into(), view_areas, separators));
		}

		// Overlay layers (index 1+)
		for layer_idx in 1..layer_count {
			if self.state.layout.layer_slot_has_layout(layer_idx) {
				let generation = self.state.layout.layer_slot_generation(layer_idx);
				let layer_id = LayerId::new(layer_idx as u16, generation);
				let layer_area = self.state.layout.layer_area(layer_id, doc_area.into());
				let view_areas = self
					.state
					.layout
					.compute_view_areas_for_layer(base_layout, layer_id, layer_area)
					.into_iter()
					.map(|(view_id, rect)| (view_id, rect.into()))
					.collect();
				let separators = self
					.state
					.layout
					.separator_positions_for_layer(base_layout, layer_id, layer_area)
					.into_iter()
					.map(|(direction, priority, rect)| (direction, priority, rect.into()))
					.collect();
				layer_data.push((layer_id, layer_area.into(), view_areas, separators));
			}
		}

		let mouse_drag_active = self.state.layout.text_selection_origin.is_some();
		for (_, _, view_areas, _) in &layer_data {
			for (buffer_id, area) in view_areas {
				let tab_width = self.tab_width_for(*buffer_id);
				let scroll_margin = if mouse_drag_active { 0 } else { self.scroll_margin_for(*buffer_id) };

				if let Some(buffer) = self.get_buffer_mut(*buffer_id) {
					let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
					let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");
					let gutter = GutterSelector::Registry;
					let effective_gutter = if is_diff_file {
						BufferRenderContext::diff_gutter_selector(gutter)
					} else {
						gutter
					};

					let gutter_layout = GutterLayout::from_selector(effective_gutter, total_lines, area.width);
					let text_width = area.width.saturating_sub(gutter_layout.total_width) as usize;

					ensure_buffer_cursor_visible(buffer, *area, text_width, tab_width, scroll_margin);
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
						syntax_manager: &self.state.syntax_manager,
						diagnostics: ctx.lsp.diagnostics_for(*buffer_id),
						diagnostic_ranges: ctx.lsp.diagnostic_ranges_for(*buffer_id),
					};
					let result = buffer_ctx.render_buffer(buffer, *area, use_block_cursor, is_focused, tab_width, cursorline, &mut cache);

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
					SplitDirection::Horizontal => (0..sep_rect.height).map(|_| Line::from(Span::styled("\u{2502}", style))).collect(),
					SplitDirection::Vertical => vec![Line::from(Span::styled("\u{2500}".repeat(sep_rect.width as usize), style))],
				};
				frame.render_widget(Paragraph::new(lines), *sep_rect);
			}

			self.render_separator_junctions(frame, separators, &sep_style);
		}
	}

	/// Renders junction glyphs where separators intersect within a layer.
	///
	/// Uses a raster-based approach: builds an occupancy map of separator cells,
	/// then derives junction connectivity from neighbor relationships. This is
	/// simpler and more correct than the previous O(n²) rectangle intersection
	/// logic with complex adjacency predicates.
	fn render_separator_junctions(&self, frame: &mut xeno_tui::Frame, separators: &[(SplitDirection, u8, Rect)], sep_style: &SeparatorStyle) {
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

			let connectivity = (up as u8) | ((down as u8) << 1) | ((left as u8) << 2) | ((right as u8) << 3);

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
