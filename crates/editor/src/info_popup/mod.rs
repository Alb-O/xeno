//! Info popup panels for displaying documentation and contextual information.
//!
//! Info popups are read-only overlay buffers used for:
//! * LSP hover documentation
//! * Command completion info in the command palette
//! * Any contextual help or documentation display
//!
//! They reuse the buffer renderer for syntax highlighting and text wrapping.

use std::collections::HashMap;

use crate::Editor;
use crate::buffer::ViewId;
use crate::geometry::Rect;
use crate::window::WindowId;

/// Unique identifier for an info popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InfoPopupId(pub(crate) u64);

impl InfoPopupId {
	pub fn as_u64(self) -> u64 {
		self.0
	}
}

/// An active info popup instance.
#[derive(Debug)]
pub struct InfoPopup {
	/// Unique identifier for this popup.
	pub id: InfoPopupId,
	/// The read-only buffer displaying content.
	pub buffer_id: ViewId,
	/// Anchor position for the popup (where it should appear relative to).
	pub anchor: PopupAnchor,
	/// Preferred content width (before border/padding).
	pub content_width: u16,
	/// Preferred content height (before border/padding).
	pub content_height: u16,
}

/// Data-only popup render target consumed by frontend scene layers.
#[derive(Debug, Clone, Copy)]
pub struct InfoPopupRenderTarget {
	pub(crate) id: InfoPopupId,
	pub(crate) buffer_id: ViewId,
	pub(crate) anchor: InfoPopupRenderAnchor,
	pub(crate) content_width: u16,
	pub(crate) content_height: u16,
}

impl InfoPopupRenderTarget {
	pub fn id(&self) -> InfoPopupId {
		self.id
	}

	pub fn buffer_id(&self) -> ViewId {
		self.buffer_id
	}

	pub fn anchor(&self) -> InfoPopupRenderAnchor {
		self.anchor
	}

	pub fn content_width(&self) -> u16 {
		self.content_width
	}

	pub fn content_height(&self) -> u16 {
		self.content_height
	}
}

/// Data-only popup layout target with resolved bounds.
#[derive(Debug, Clone, Copy)]
pub struct InfoPopupLayoutTarget {
	/// Stable popup identifier.
	pub id: InfoPopupId,
	/// Read-only popup buffer to render.
	pub buffer_id: ViewId,
	/// Resolved popup rectangle in grid coordinates (outer bounds including padding).
	pub rect: Rect,
	/// Content area after applying padding (where buffer content is rendered).
	pub inner_rect: Rect,
	/// Anchor placement strategy used to derive `rect`.
	pub anchor: InfoPopupRenderAnchor,
}

impl From<&InfoPopup> for InfoPopupRenderTarget {
	fn from(popup: &InfoPopup) -> Self {
		Self {
			id: popup.id,
			buffer_id: popup.buffer_id,
			anchor: popup.anchor.into(),
			content_width: popup.content_width,
			content_height: popup.content_height,
		}
	}
}

/// Render-only popup anchor consumed by frontend layers.
#[derive(Debug, Clone, Copy)]
pub enum InfoPopupRenderAnchor {
	/// Centered in the document area.
	Center,
	/// Position relative to a specific screen coordinate (top-left of popup).
	Point { x: u16, y: u16 },
	/// Centered within a specific window's area.
	Window(WindowId),
}

impl From<PopupAnchor> for InfoPopupRenderAnchor {
	fn from(anchor: PopupAnchor) -> Self {
		match anchor {
			PopupAnchor::Center => Self::Center,
			PopupAnchor::Point { x, y } => Self::Point { x, y },
			PopupAnchor::Window(wid) => Self::Window(wid),
		}
	}
}

/// Anchor point for positioning info popups.
#[derive(Debug, Clone, Copy, Default)]
pub enum PopupAnchor {
	/// Centered in the document area.
	#[default]
	Center,
	/// Position relative to a specific screen coordinate (top-left of popup).
	Point { x: u16, y: u16 },
	/// Position adjacent to another window (e.g., next to completion menu).
	Window(WindowId),
}

fn measure_content(content: &str) -> (u16, u16) {
	let lines: Vec<&str> = content.lines().collect();
	let content_height = lines.len().min(20) as u16;
	let content_width = lines.iter().map(|line| line.chars().count()).max().unwrap_or(20).min(60) as u16;
	(content_width, content_height)
}

const MAX_CONTENT_W: u16 = 60;
const MAX_CONTENT_H: u16 = 12;
const POPUP_H_PADDING: u16 = 1;

/// Computes the content rect inside a popup's outer rect by applying horizontal padding.
pub fn popup_inner_rect(rect: Rect) -> Rect {
	Rect::new(
		rect.x.saturating_add(POPUP_H_PADDING),
		rect.y,
		rect.width.saturating_sub(POPUP_H_PADDING * 2),
		rect.height,
	)
}

/// Returns the intersection of two rects, or a zero-size rect if they don't overlap.
fn intersect_rect(a: Rect, b: Rect) -> Rect {
	let x1 = a.x.max(b.x);
	let y1 = a.y.max(b.y);
	let x2 = (a.x as u32 + a.width as u32).min(b.x as u32 + b.width as u32);
	let y2 = (a.y as u32 + a.height as u32).min(b.y as u32 + b.height as u32);
	if x2 <= x1 as u32 || y2 <= y1 as u32 {
		return Rect::new(a.x, a.y, 0, 0);
	}
	Rect::new(x1, y1, (x2 - x1 as u32) as u16, (y2 - y1 as u32) as u16)
}

/// Computes the popup rectangle within `frame`, clamped to `bounds`.
///
/// `frame` is the region used for centering (e.g. a specific window area).
/// `bounds` is the hard outer boundary the popup must not escape.
fn compute_popup_rect(anchor: InfoPopupRenderAnchor, content_width: u16, content_height: u16, frame: Rect, bounds: Rect) -> Option<Rect> {
	let max_w = bounds.width.saturating_sub(2).min(MAX_CONTENT_W);
	let max_h = bounds.height.saturating_sub(2).min(MAX_CONTENT_H);
	if max_w == 0 || max_h == 0 {
		return None;
	}

	let width = content_width.min(max_w);
	let height = content_height.min(max_h);
	if width == 0 || height == 0 {
		return None;
	}

	let outer_w = width.saturating_add(2).min(bounds.width.saturating_sub(4));
	let outer_h = height.saturating_add(2).min(bounds.height.saturating_sub(2));
	if outer_w == 0 || outer_h == 0 {
		return None;
	}

	let (x, y) = match anchor {
		InfoPopupRenderAnchor::Center | InfoPopupRenderAnchor::Window(_) => (
			frame.x + frame.width.saturating_sub(outer_w) / 2,
			frame.y + frame.height.saturating_sub(outer_h) / 2,
		),
		InfoPopupRenderAnchor::Point { x, y } => (x, y),
	};

	// Clamp to bounds.
	let x = x.max(bounds.x).min(bounds.x + bounds.width.saturating_sub(outer_w));
	let y = y.max(bounds.y).min(bounds.y + bounds.height.saturating_sub(outer_h));

	Some(Rect::new(x, y, outer_w, outer_h))
}

/// Storage for active info popups, keyed by [`InfoPopupId`].
///
/// Stored in the type-erased overlay store to avoid adding fields to [`Editor`].
#[derive(Default)]
pub struct InfoPopupStore {
	popups: HashMap<InfoPopupId, InfoPopup>,
	next_id: u64,
}

impl InfoPopupStore {
	/// Allocates a new unique popup ID.
	pub fn next_id(&mut self) -> InfoPopupId {
		let id = InfoPopupId(self.next_id);
		self.next_id += 1;
		id
	}

	/// Inserts a popup into the store.
	pub fn insert(&mut self, popup: InfoPopup) {
		self.popups.insert(popup.id, popup);
	}

	/// Removes and returns a popup by ID.
	pub fn remove(&mut self, id: InfoPopupId) -> Option<InfoPopup> {
		self.popups.remove(&id)
	}

	/// Returns a reference to a popup by ID.
	pub fn get(&self, id: InfoPopupId) -> Option<&InfoPopup> {
		self.popups.get(&id)
	}

	/// Returns a mutable reference to a popup by ID.
	pub fn get_mut(&mut self, id: InfoPopupId) -> Option<&mut InfoPopup> {
		self.popups.get_mut(&id)
	}

	/// Returns an iterator over all popup IDs.
	pub fn ids(&self) -> impl Iterator<Item = InfoPopupId> + '_ {
		self.popups.keys().copied()
	}

	/// Returns the number of active popups.
	pub fn len(&self) -> usize {
		self.popups.len()
	}

	/// Builds a data-only render plan for all active popups.
	pub fn render_plan(&self) -> Vec<InfoPopupRenderTarget> {
		let mut plan: Vec<_> = self.popups.values().map(InfoPopupRenderTarget::from).collect();
		plan.sort_by_key(|popup| popup.id.0);
		plan
	}

	/// Returns true if there are no active popups.
	pub fn is_empty(&self) -> bool {
		self.popups.is_empty()
	}
}

impl Editor {
	/// Opens an info popup with the given content.
	///
	/// The popup is positioned relative to the anchor point. Content is displayed
	/// in a read-only buffer with syntax highlighting based on the optional file type.
	pub fn open_info_popup(&mut self, content: String, file_type: Option<&str>, anchor: PopupAnchor) -> Option<InfoPopupId> {
		self.state.viewport.doc_area?;
		let (content_width, content_height) = measure_content(content.as_str());

		let buffer_id = self.state.core.buffers.create_scratch();
		{
			let buffer = self.state.core.buffers.get_buffer_mut(buffer_id).expect("just created");
			buffer.reset_content(content.as_str());
			if let Some(ft) = file_type {
				buffer.with_doc_mut(|doc| doc.init_syntax_for_language(ft, &self.state.config.language_loader));
			}
			self.state.syntax_manager.reset_syntax(buffer.document_id());
			buffer.set_readonly_override(Some(true));
		}

		let store = self.overlays_mut().get_or_default::<InfoPopupStore>();
		let popup_id = store.next_id();
		store.insert(InfoPopup {
			id: popup_id,
			buffer_id,
			anchor,
			content_width,
			content_height,
		});

		self.state.frame.needs_redraw = true;
		Some(popup_id)
	}

	/// Closes an info popup by ID.
	pub fn close_info_popup(&mut self, popup_id: InfoPopupId) {
		let Some(popup) = self.overlays_mut().get_or_default::<InfoPopupStore>().remove(popup_id) else {
			return;
		};
		self.finalize_buffer_removal(popup.buffer_id);
		self.state.frame.needs_redraw = true;
	}

	/// Closes all open info popups.
	pub fn close_all_info_popups(&mut self) {
		let popup_ids: Vec<_> = self.overlays_mut().get_or_default::<InfoPopupStore>().ids().collect();
		for id in popup_ids {
			self.close_info_popup(id);
		}
	}

	/// Updates the content of an existing info popup.
	pub fn update_info_popup(&mut self, popup_id: InfoPopupId, content: String, file_type: Option<&str>) -> bool {
		let Some(buffer_id) = self.overlays_mut().get_or_default::<InfoPopupStore>().get_mut(popup_id).map(|p| {
			let (content_width, content_height) = measure_content(content.as_str());
			p.content_width = content_width;
			p.content_height = content_height;
			p.buffer_id
		}) else {
			return false;
		};

		let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) else {
			return false;
		};

		buffer.set_readonly_override(None);
		buffer.reset_content(content.as_str());

		if let Some(ft) = file_type {
			let current_ft = buffer.file_type();
			if current_ft.as_deref() != Some(ft) {
				buffer.with_doc_mut(|doc| doc.init_syntax_for_language(ft, &self.state.config.language_loader));
			}
		}

		self.state.syntax_manager.reset_syntax(buffer.document_id());
		buffer.set_readonly_override(Some(true));
		self.state.frame.needs_redraw = true;
		true
	}

	/// Returns the number of open info popups.
	pub fn info_popup_count(&self) -> usize {
		self.overlays().get::<InfoPopupStore>().map_or(0, |s: &InfoPopupStore| s.len())
	}

	/// Returns a data-only render plan for active info popups.
	pub fn info_popup_render_plan(&self) -> Vec<InfoPopupRenderTarget> {
		self.overlays().get::<InfoPopupStore>().map_or_else(Vec::new, InfoPopupStore::render_plan)
	}

	/// Returns info popup layout targets with resolved and clamped rectangles.
	///
	/// `bounds` is the document area used as both the default centering frame
	/// and the hard outer boundary. For `Window` anchors, the frame is the
	/// target window's view area (intersected with bounds).
	pub fn info_popup_layout_plan(&self, bounds: Rect) -> Vec<InfoPopupLayoutTarget> {
		self.info_popup_render_plan()
			.into_iter()
			.filter_map(|popup| {
				let frame = self.resolve_popup_frame(popup.anchor, bounds);
				let rect = compute_popup_rect(popup.anchor, popup.content_width, popup.content_height, frame, bounds)?;
				let inner_rect = popup_inner_rect(rect);
				Some(InfoPopupLayoutTarget {
					id: popup.id,
					buffer_id: popup.buffer_id,
					rect,
					inner_rect,
					anchor: popup.anchor,
				})
			})
			.collect()
	}

	/// Resolves the centering frame for a popup anchor.
	fn resolve_popup_frame(&self, anchor: InfoPopupRenderAnchor, bounds: Rect) -> Rect {
		match anchor {
			InfoPopupRenderAnchor::Window(wid) => {
				// Use the focused view area of the target window, intersected with bounds.
				self.state
					.windows
					.get(wid)
					.map(|window| {
						let view_id = window.buffer();
						let area = self.view_area(view_id);
						intersect_rect(area, bounds)
					})
					.unwrap_or(bounds)
			}
			_ => bounds,
		}
	}
}

#[cfg(test)]
mod tests;
