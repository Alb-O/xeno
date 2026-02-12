//! Info popup panels for displaying documentation and contextual information.
//!
//! Info popups are read-only overlay buffers used for:
//! - LSP hover documentation
//! - Command completion info in the command palette
//! - Any contextual help or documentation display
//!
//! They reuse the buffer renderer for syntax highlighting and text wrapping.

use std::collections::HashMap;

use crate::buffer::ViewId;
use crate::window::WindowId;
use crate::Editor;

/// Unique identifier for an info popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InfoPopupId(pub u64);

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
	/// Stable popup identifier.
	pub id: InfoPopupId,
	/// Read-only popup buffer to render.
	pub buffer_id: ViewId,
	/// Anchor placement strategy for this popup.
	pub anchor: PopupAnchor,
	/// Preferred content width before chrome.
	pub content_width: u16,
	/// Preferred content height before chrome.
	pub content_height: u16,
}

impl From<&InfoPopup> for InfoPopupRenderTarget {
	fn from(popup: &InfoPopup) -> Self {
		Self {
			id: popup.id,
			buffer_id: popup.buffer_id,
			anchor: popup.anchor,
			content_width: popup.content_width,
			content_height: popup.content_height,
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
		self.popups.values().map(InfoPopupRenderTarget::from).collect()
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
}

#[cfg(test)]
mod tests;
