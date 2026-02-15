//! Resolved view-plan assembly for documents, overlays, popups, and separators.
//!
//! Produces frontend-facing data-only plans with concrete geometry and rendered
//! lines, while keeping layout/render policy in the core editor.

use super::{BufferRenderContext, GutterLayout, RenderLine, ensure_buffer_cursor_visible};
use crate::Editor;
use crate::buffer::{SplitDirection, ViewId};
use crate::geometry::Rect;
use crate::info_popup::InfoPopupId;
use crate::layout::LayerId;
use crate::overlay::WindowRole;
use crate::window::{GutterSelector, SurfaceStyle};

#[derive(Debug, Clone)]
pub(crate) struct BufferViewRenderPlan {
	#[cfg(test)]
	pub(crate) gutter_width: u16,
	pub(crate) gutter_rect: Rect,
	pub(crate) text_rect: Rect,
	pub(crate) gutter: Vec<RenderLine<'static>>,
	pub(crate) text: Vec<RenderLine<'static>>,
}

impl Editor {
	/// Returns the title string for the focused document (path or `"[scratch]"`).
	pub fn focused_document_title(&self) -> String {
		self.get_buffer(self.focused_view())
			.and_then(|buffer| buffer.path().as_ref().map(|path| path.display().to_string()))
			.unwrap_or_else(|| String::from("[scratch]"))
	}

	/// Renders a single view into data-only gutter and text lines.
	pub(crate) fn buffer_view_render_plan(&mut self, view: ViewId, area: Rect, use_block_cursor: bool, is_focused: bool) -> Option<BufferViewRenderPlan> {
		self.buffer_view_render_plan_with_gutter(view, area, use_block_cursor, is_focused, crate::window::GutterSelector::Registry)
	}

	/// Renders a single view into data-only gutter and text lines with an explicit gutter policy.
	pub(crate) fn buffer_view_render_plan_with_gutter(
		&mut self,
		view: ViewId,
		area: Rect,
		use_block_cursor: bool,
		is_focused: bool,
		gutter: crate::window::GutterSelector,
	) -> Option<BufferViewRenderPlan> {
		self.ensure_syntax_for_buffers();
		if area.width == 0 || area.height == 0 {
			return None;
		}

		let tab_width = self.tab_width_for(view);
		let mouse_drag_active = self.layout().text_selection_origin.is_some();
		let scroll_margin = if mouse_drag_active { 0 } else { self.scroll_margin_for(view) };

		{
			let buffer = self.get_buffer_mut(view)?;
			let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
			let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");
			let effective_gutter = if is_diff_file {
				BufferRenderContext::diff_gutter_selector(gutter)
			} else {
				gutter
			};

			let gutter_layout = GutterLayout::from_selector(effective_gutter, total_lines, area.width);
			let text_width = area.width.saturating_sub(gutter_layout.total_width) as usize;
			ensure_buffer_cursor_visible(buffer, area, text_width, tab_width, scroll_margin);
		}

		let render_ctx = self.render_ctx();
		let mut cache = std::mem::take(self.render_cache_mut());
		let cursorline = self.cursorline_for(view);

		let buffer = self.get_buffer(view)?;
		let buffer_ctx = BufferRenderContext {
			theme: &render_ctx.theme,
			language_loader: &self.config().language_loader,
			syntax_manager: self.syntax_manager(),
			diagnostics: render_ctx.lsp.diagnostics_for(view),
			diagnostic_ranges: render_ctx.lsp.diagnostic_ranges_for(view),
		};

		let result = buffer_ctx.render_buffer(buffer, area, use_block_cursor, is_focused, tab_width, cursorline, &mut cache);
		*self.render_cache_mut() = cache;

		let gutter_width = result.gutter_width.min(area.width);
		let gutter_rect = Rect::new(area.x, area.y, gutter_width, area.height);
		let text_rect = Rect::new(area.x + gutter_width, area.y, area.width - gutter_width, area.height);

		Some(BufferViewRenderPlan {
			#[cfg(test)]
			gutter_width,
			gutter_rect,
			text_rect,
			gutter: result.gutter,
			text: result.text,
		})
	}
}

/// Fully resolved overlay pane with pre-rendered content lines.
#[derive(Debug, Clone)]
pub struct OverlayPaneViewPlan {
	role: WindowRole,
	rect: Rect,
	content_rect: Rect,
	style: SurfaceStyle,
	gutter_rect: Rect,
	text_rect: Rect,
	gutter: Vec<RenderLine<'static>>,
	text: Vec<RenderLine<'static>>,
}

impl OverlayPaneViewPlan {
	pub fn role(&self) -> WindowRole {
		self.role
	}
	pub fn rect(&self) -> Rect {
		self.rect
	}
	pub fn content_rect(&self) -> Rect {
		self.content_rect
	}
	pub fn style(&self) -> &SurfaceStyle {
		&self.style
	}
	pub fn gutter_rect(&self) -> Rect {
		self.gutter_rect
	}
	pub fn text_rect(&self) -> Rect {
		self.text_rect
	}
	pub fn gutter(&self) -> &[RenderLine<'static>] {
		&self.gutter
	}
	pub fn text(&self) -> &[RenderLine<'static>] {
		&self.text
	}
}

/// Fully resolved info popup with pre-rendered content lines.
#[derive(Debug, Clone)]
pub struct InfoPopupViewPlan {
	id: InfoPopupId,
	rect: Rect,
	inner_rect: Rect,
	gutter_rect: Rect,
	text_rect: Rect,
	gutter: Vec<RenderLine<'static>>,
	text: Vec<RenderLine<'static>>,
}

impl InfoPopupViewPlan {
	pub fn id(&self) -> InfoPopupId {
		self.id
	}
	pub fn rect(&self) -> Rect {
		self.rect
	}
	pub fn inner_rect(&self) -> Rect {
		self.inner_rect
	}
	pub fn gutter_rect(&self) -> Rect {
		self.gutter_rect
	}
	pub fn text_rect(&self) -> Rect {
		self.text_rect
	}
	pub fn gutter(&self) -> &[RenderLine<'static>] {
		&self.gutter
	}
	pub fn text(&self) -> &[RenderLine<'static>] {
		&self.text
	}
}

impl Editor {
	/// Returns fully resolved overlay pane view plans with pre-rendered content.
	///
	/// Core decides cursor style, focus, and gutter policy. Frontends only draw.
	pub fn overlay_pane_view_plans(&mut self) -> Vec<OverlayPaneViewPlan> {
		let panes = self.overlay_pane_render_plan();
		let focused_overlay = match self.focus() {
			crate::FocusTarget::Overlay { buffer } => Some(*buffer),
			_ => None,
		};

		panes
			.into_iter()
			.filter_map(|pane| {
				let content_rect = pane.content_rect;
				if content_rect.width == 0 || content_rect.height == 0 {
					return None;
				}

				let render = self.buffer_view_render_plan_with_gutter(pane.buffer, content_rect, true, focused_overlay == Some(pane.buffer), pane.gutter)?;

				Some(OverlayPaneViewPlan {
					role: pane.role,
					rect: pane.rect,
					content_rect,
					style: pane.style,
					gutter_rect: render.gutter_rect,
					text_rect: render.text_rect,
					gutter: render.gutter,
					text: render.text,
				})
			})
			.collect()
	}

	/// Returns fully resolved info popup view plans with pre-rendered content.
	///
	/// Core decides placement, padding, cursor/focus flags, and gutter policy.
	pub fn info_popup_view_plans(&mut self, bounds: Rect) -> Vec<InfoPopupViewPlan> {
		let targets = self.info_popup_layout_plan(bounds);

		targets
			.into_iter()
			.filter_map(|target| {
				if target.rect.width == 0 || target.rect.height == 0 {
					return None;
				}

				let inner = target.inner_rect;
				if inner.width == 0 || inner.height == 0 {
					return None;
				}

				let render = self.buffer_view_render_plan_with_gutter(target.buffer_id, inner, false, false, GutterSelector::Hidden)?;

				Some(InfoPopupViewPlan {
					id: target.id,
					rect: target.rect,
					inner_rect: inner,
					gutter_rect: render.gutter_rect,
					text_rect: render.text_rect,
					gutter: render.gutter,
					text: render.text,
				})
			})
			.collect()
	}
}

/// Fully resolved document view with pre-rendered content lines.
///
/// One plan per visible view across all layout layers (base + overlay).
#[derive(Debug, Clone)]
pub struct DocumentViewPlan {
	view: ViewId,
	rect: Rect,
	gutter_rect: Rect,
	text_rect: Rect,
	gutter: Vec<RenderLine<'static>>,
	text: Vec<RenderLine<'static>>,
}

impl DocumentViewPlan {
	pub fn view(&self) -> ViewId {
		self.view
	}
	pub fn rect(&self) -> Rect {
		self.rect
	}
	pub fn gutter_rect(&self) -> Rect {
		self.gutter_rect
	}
	pub fn text_rect(&self) -> Rect {
		self.text_rect
	}
	pub fn gutter(&self) -> &[RenderLine<'static>] {
		&self.gutter
	}
	pub fn text(&self) -> &[RenderLine<'static>] {
		&self.text
	}
}

/// Separator state for frontend styling decisions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorState {
	pub(crate) is_hovered: bool,
	pub(crate) is_dragging: bool,
	pub(crate) is_animating: bool,
	pub(crate) anim_intensity: f32,
}

impl SeparatorState {
	pub fn is_hovered(&self) -> bool {
		self.is_hovered
	}
	pub fn is_dragging(&self) -> bool {
		self.is_dragging
	}
	pub fn is_animating(&self) -> bool {
		self.is_animating
	}
	pub fn anim_intensity(&self) -> f32 {
		self.anim_intensity
	}
}

/// Fully resolved separator with geometry and interaction state.
#[derive(Debug, Clone)]
pub struct SeparatorRenderTarget {
	pub(crate) direction: SplitDirection,
	pub(crate) priority: u8,
	pub(crate) rect: Rect,
	pub(crate) state: SeparatorState,
}

impl SeparatorRenderTarget {
	pub fn direction(&self) -> SplitDirection {
		self.direction
	}
	pub fn priority(&self) -> u8 {
		self.priority
	}
	pub fn rect(&self) -> Rect {
		self.rect
	}
	pub fn state(&self) -> &SeparatorState {
		&self.state
	}
}

/// Junction glyph at a separator intersection point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorJunctionTarget {
	pub(crate) x: u16,
	pub(crate) y: u16,
	pub(crate) glyph: char,
	pub(crate) priority: u8,
	pub(crate) state: SeparatorState,
}

impl SeparatorJunctionTarget {
	pub fn x(&self) -> u16 {
		self.x
	}
	pub fn y(&self) -> u16 {
		self.y
	}
	pub fn glyph(&self) -> char {
		self.glyph
	}
	pub fn priority(&self) -> u8 {
		self.priority
	}
	pub fn state(&self) -> &SeparatorState {
		&self.state
	}
}

/// Combined separator scene: segments and their junctions, computed in a single pass.
#[derive(Debug, Clone)]
pub struct SeparatorScenePlan {
	pub(crate) separators: Vec<SeparatorRenderTarget>,
	pub(crate) junctions: Vec<SeparatorJunctionTarget>,
}

impl SeparatorScenePlan {
	pub fn separators(&self) -> &[SeparatorRenderTarget] {
		&self.separators
	}
	pub fn junctions(&self) -> &[SeparatorJunctionTarget] {
		&self.junctions
	}
}

/// Returns the box-drawing junction glyph for the given connectivity mask.
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

/// Computes junction targets from separator render targets.
///
/// Uses a raster-based occupancy map: rasterizes all separator cells, then
/// derives junction connectivity from neighbor relationships.
pub(crate) fn compute_junction_targets(targets: &[SeparatorRenderTarget]) -> Vec<SeparatorJunctionTarget> {
	use std::collections::HashMap;

	#[derive(Debug, Clone, Copy, Default)]
	struct CellOcc {
		has_h: bool,
		has_v: bool,
		prio: u8,
		state: Option<SeparatorState>,
	}

	let mut occ: HashMap<(u16, u16), CellOcc> = HashMap::new();

	for target in targets {
		match target.direction {
			SplitDirection::Horizontal => {
				let x = target.rect.x;
				for y in target.rect.y..target.rect.bottom() {
					let cell = occ.entry((x, y)).or_default();
					cell.has_v = true;
					cell.prio = cell.prio.max(target.priority);
					merge_state(&mut cell.state, &target.state);
				}
			}
			SplitDirection::Vertical => {
				let y = target.rect.y;
				for x in target.rect.x..target.rect.right() {
					let cell = occ.entry((x, y)).or_default();
					cell.has_h = true;
					cell.prio = cell.prio.max(target.priority);
					merge_state(&mut cell.state, &target.state);
				}
			}
		}
	}

	let mut junctions = Vec::new();

	for (&(x, y), cell) in &occ {
		let has_up = y > 0 && occ.get(&(x, y - 1)).is_some_and(|c| c.has_v);
		let has_down = occ.get(&(x, y.saturating_add(1))).is_some_and(|c| c.has_v);
		let has_left = x > 0 && occ.get(&(x - 1, y)).is_some_and(|c| c.has_h);
		let has_right = occ.get(&(x.saturating_add(1), y)).is_some_and(|c| c.has_h);

		let up = cell.has_v || has_up;
		let down = cell.has_v || has_down;
		let left = cell.has_h || has_left;
		let right = cell.has_h || has_right;

		let is_junction = (has_down || has_up || cell.has_v) && cell.has_h;
		if !is_junction {
			continue;
		}

		let connectivity = (up as u8) | ((down as u8) << 1) | ((left as u8) << 2) | ((right as u8) << 3);
		if connectivity == 0b0011 {
			continue; // Just a vertical line segment.
		}

		let state = cell.state.unwrap_or(SeparatorState {
			is_hovered: false,
			is_dragging: false,
			is_animating: false,
			anim_intensity: 0.0,
		});

		junctions.push(SeparatorJunctionTarget {
			x,
			y,
			glyph: junction_glyph(connectivity),
			priority: cell.prio,
			state,
		});
	}

	junctions.sort_by_key(|j| (j.y, j.x));
	junctions
}

/// Merges separator state, preferring the most active state (dragging > animating > hovered).
fn merge_state(existing: &mut Option<SeparatorState>, incoming: &SeparatorState) {
	let Some(ex) = existing else {
		*existing = Some(*incoming);
		return;
	};
	if incoming.is_dragging {
		ex.is_dragging = true;
	}
	if incoming.is_animating {
		ex.is_animating = true;
		ex.anim_intensity = ex.anim_intensity.max(incoming.anim_intensity);
	}
	if incoming.is_hovered {
		ex.is_hovered = true;
	}
}

impl Editor {
	/// Returns the full separator scene (segments + junctions) in a single pass.
	///
	/// Preferred over calling `separator_render_targets` and `separator_junction_targets`
	/// separately, since it avoids recomputing the separator list.
	pub fn separator_scene_plan(&mut self, doc_area: Rect) -> SeparatorScenePlan {
		let separators = self.separator_render_targets(doc_area);
		let junctions = compute_junction_targets(&separators);
		SeparatorScenePlan { separators, junctions }
	}

	/// Returns document view plans for all visible views across layout layers.
	///
	/// Core decides cursor style, focus, and gutter policy. Frontends only draw.
	pub fn document_view_plans(&mut self, doc_area: Rect) -> Vec<DocumentViewPlan> {
		let use_block_cursor = matches!(self.derive_cursor_style(), crate::runtime::CursorStyle::Block);
		let focused_view = self.focused_view();
		let base_layout = self.base_window().layout.clone();
		let layer_count = self.layout().layer_count();

		let mut all_views: Vec<(ViewId, Rect)> = Vec::new();

		// Base layer
		{
			let layer_id = LayerId::BASE;
			let layer_area = self.layout().layer_area(layer_id, doc_area);
			let views = self.layout().compute_view_areas_for_layer(&base_layout, layer_id, layer_area);
			all_views.extend(views);
		}

		// Overlay layers
		for layer_idx in 1..layer_count {
			if self.layout().layer_slot_has_layout(layer_idx) {
				let generation = self.layout().layer_slot_generation(layer_idx);
				let layer_id = LayerId::new(layer_idx as u16, generation);
				let layer_area = self.layout().layer_area(layer_id, doc_area);
				let views = self.layout().compute_view_areas_for_layer(&base_layout, layer_id, layer_area);
				all_views.extend(views);
			}
		}

		all_views
			.into_iter()
			.filter_map(|(view, rect)| {
				if rect.width == 0 || rect.height == 0 {
					return None;
				}
				let is_focused = view == focused_view;
				let render = self.buffer_view_render_plan(view, rect, use_block_cursor, is_focused)?;
				Some(DocumentViewPlan {
					view,
					rect,
					gutter_rect: render.gutter_rect,
					text_rect: render.text_rect,
					gutter: render.gutter,
					text: render.text,
				})
			})
			.collect()
	}

	/// Returns separator render targets for all visible separators across layout layers.
	///
	/// Core resolves geometry and interaction state (hover/drag/animation).
	/// Frontends use the state to choose colors and render glyphs.
	pub(crate) fn separator_render_targets(&mut self, doc_area: Rect) -> Vec<SeparatorRenderTarget> {
		let base_layout = self.base_window().layout.clone();
		let layer_count = self.layout().layer_count();

		let hovered_rect: Option<Rect> = self.layout().hovered_separator.map(|(_, r)| r);
		let dragging_rect: Option<Rect> = self
			.layout()
			.drag_state()
			.and_then(|drag| self.layout().separator_rect(&base_layout, doc_area, &drag.id));
		let anim_rect: Option<Rect> = self.layout().animation_rect();
		let anim_intensity = self.layout().animation_intensity();

		let mut all_seps: Vec<(SplitDirection, u8, Rect)> = Vec::new();

		// Base layer
		{
			let layer_id = LayerId::BASE;
			let layer_area = self.layout().layer_area(layer_id, doc_area);
			let seps = self.layout().separator_positions_for_layer(&base_layout, layer_id, layer_area);
			all_seps.extend(seps);
		}

		// Overlay layers
		for layer_idx in 1..layer_count {
			if self.layout().layer_slot_has_layout(layer_idx) {
				let generation = self.layout().layer_slot_generation(layer_idx);
				let layer_id = LayerId::new(layer_idx as u16, generation);
				let layer_area = self.layout().layer_area(layer_id, doc_area);
				let seps = self.layout().separator_positions_for_layer(&base_layout, layer_id, layer_area);
				all_seps.extend(seps);
			}
		}

		all_seps
			.into_iter()
			.map(|(direction, priority, rect)| {
				let is_hovered = hovered_rect == Some(rect);
				let is_dragging = dragging_rect == Some(rect);
				let is_animating = anim_rect == Some(rect);
				SeparatorRenderTarget {
					direction,
					priority,
					rect,
					state: SeparatorState {
						is_hovered,
						is_dragging,
						is_animating,
						anim_intensity: if is_animating { anim_intensity } else { 0.0 },
					},
				}
			})
			.collect()
	}
}

#[cfg(test)]
mod tests;
