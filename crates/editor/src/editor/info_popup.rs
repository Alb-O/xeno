//! Info popup integration with editor.

use super::Editor;
use crate::info_popup::{
	InfoPopup, InfoPopupId, InfoPopupStore, PopupAnchor, compute_popup_rect, info_popup_style,
};
use crate::window::{GutterSelector, Window};

impl Editor {
	/// Opens an info popup with the given content.
	///
	/// The popup is positioned relative to the anchor point. Content is displayed
	/// in a read-only buffer with syntax highlighting based on the optional file type.
	pub fn open_info_popup(
		&mut self,
		content: String,
		file_type: Option<&str>,
		anchor: PopupAnchor,
	) -> Option<InfoPopupId> {
		let bounds = self.viewport.doc_area?;

		let lines: Vec<&str> = content.lines().collect();
		let content_height = lines.len().min(20) as u16;
		let content_width = lines
			.iter()
			.map(|l| l.chars().count())
			.max()
			.unwrap_or(20)
			.min(60) as u16;

		let rect = compute_popup_rect(anchor, content_width, content_height, bounds);

		let buffer_id = self.buffers.create_scratch();
		{
			let buffer = self
				.buffers
				.get_buffer_mut(buffer_id)
				.expect("just created");
			buffer.doc_mut().content = ropey::Rope::from_str(&content);
			if let Some(ft) = file_type {
				buffer
					.doc_mut()
					.init_syntax_for_language(ft, &self.config.language_loader);
			}
			buffer.set_readonly_override(Some(true));
		}

		let window_id = self.create_floating_window(buffer_id, rect, info_popup_style());

		let Window::Floating(float) = self.windows.get_mut(window_id).expect("just created") else {
			unreachable!()
		};
		float.sticky = false;
		float.dismiss_on_blur = true;
		float.gutter = GutterSelector::Hidden;

		let store = self.overlays.get_or_default::<InfoPopupStore>();
		let popup_id = store.next_id();
		store.insert(InfoPopup {
			id: popup_id,
			window_id,
			buffer_id,
			anchor,
		});

		self.frame.needs_redraw = true;
		Some(popup_id)
	}

	/// Closes an info popup by ID.
	pub fn close_info_popup(&mut self, popup_id: InfoPopupId) {
		let Some(popup) = self
			.overlays
			.get_or_default::<InfoPopupStore>()
			.remove(popup_id)
		else {
			return;
		};
		self.close_floating_window(popup.window_id);
		self.buffers.remove_buffer(popup.buffer_id);
		self.frame.needs_redraw = true;
	}

	/// Closes all open info popups.
	pub fn close_all_info_popups(&mut self) {
		let popup_ids: Vec<_> = self
			.overlays
			.get_or_default::<InfoPopupStore>()
			.ids()
			.collect();
		for id in popup_ids {
			self.close_info_popup(id);
		}
	}

	/// Updates the content of an existing info popup.
	pub fn update_info_popup(
		&mut self,
		popup_id: InfoPopupId,
		content: String,
		file_type: Option<&str>,
	) -> bool {
		let Some(buffer_id) = self
			.overlays
			.get::<InfoPopupStore>()
			.and_then(|s| s.get(popup_id))
			.map(|p| p.buffer_id)
		else {
			return false;
		};

		let Some(buffer) = self.buffers.get_buffer_mut(buffer_id) else {
			return false;
		};

		buffer.set_readonly_override(Some(false));
		buffer.doc_mut().content = ropey::Rope::from_str(&content);

		if let Some(ft) = file_type {
			let current_ft = buffer.doc().file_type.clone();
			if current_ft.as_deref() != Some(ft) {
				buffer
					.doc_mut()
					.init_syntax_for_language(ft, &self.config.language_loader);
			}
		}

		buffer.set_readonly_override(Some(true));
		self.frame.needs_redraw = true;
		true
	}

	/// Returns the number of open info popups.
	pub fn info_popup_count(&self) -> usize {
		self.overlays.get::<InfoPopupStore>().map_or(0, |s| s.len())
	}
}
