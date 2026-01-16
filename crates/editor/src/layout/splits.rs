//! Split creation and removal.
//!
//! Creating horizontal/vertical splits and removing views from the layout.

use xeno_tui::layout::Rect;

use super::manager::LayoutManager;
use crate::buffer::{BufferId, BufferView, Layout};

impl LayoutManager {
	/// Creates a horizontal split with a new buffer below the current view.
	pub fn split_horizontal(
		&mut self,
		base_layout: &mut Layout,
		current_view: BufferView,
		new_buffer_id: BufferId,
		doc_area: Rect,
	) {
		let Some(view_area) = self.view_area(base_layout, current_view, doc_area) else {
			return;
		};
		let new_layout = Layout::stacked(
			Layout::single(current_view),
			Layout::text(new_buffer_id),
			view_area,
		);
		if let Some(layer_idx) = self.layer_of_view(base_layout, current_view) {
			if layer_idx == 0 {
				base_layout.replace_view(current_view, new_layout);
			} else if let Some(layout) = self.layer_mut(base_layout, layer_idx) {
				layout.replace_view(current_view, new_layout);
			}
		}
	}

	/// Creates a vertical split with a new buffer to the right of the current view.
	pub fn split_vertical(
		&mut self,
		base_layout: &mut Layout,
		current_view: BufferView,
		new_buffer_id: BufferId,
		doc_area: Rect,
	) {
		let Some(view_area) = self.view_area(base_layout, current_view, doc_area) else {
			return;
		};
		let new_layout = Layout::side_by_side(
			Layout::single(current_view),
			Layout::text(new_buffer_id),
			view_area,
		);
		if let Some(layer_idx) = self.layer_of_view(base_layout, current_view) {
			if layer_idx == 0 {
				base_layout.replace_view(current_view, new_layout);
			} else if let Some(layout) = self.layer_mut(base_layout, layer_idx) {
				layout.replace_view(current_view, new_layout);
			}
		}
	}

	/// Gets the area of a specific view.
	pub(super) fn view_area(
		&self,
		base_layout: &Layout,
		view: BufferView,
		doc_area: Rect,
	) -> Option<Rect> {
		let layer_idx = self.layer_of_view(base_layout, view)?;
		let layer_area = self.layer_area(layer_idx, doc_area);
		self.layer(base_layout, layer_idx)?
			.compute_view_areas(layer_area)
			.into_iter()
			.find(|(v, _)| *v == view)
			.map(|(_, area)| area)
	}

	/// Removes a view from its layer, collapsing splits as needed.
	///
	/// Returns the new focused view if the layout was modified.
	pub fn remove_view(
		&mut self,
		base_layout: &mut Layout,
		view: BufferView,
	) -> Option<BufferView> {
		let layer_idx = self.layer_of_view(base_layout, view)?;

		// Don't remove the last view from base layer
		if layer_idx == 0 && base_layout.count() <= 1 {
			return None;
		}

		if layer_idx == 0 {
			let new_layout = base_layout.remove_view(view)?;
			*base_layout = new_layout;
			return Some(base_layout.first_view());
		}

		let layout = self.layers[layer_idx].as_ref()?;
		let new_layout = layout.remove_view(view);

		if let Some(new_layout) = new_layout {
			self.layers[layer_idx] = Some(new_layout);
			Some(self.layers[layer_idx].as_ref().unwrap().first_view())
		} else {
			self.layers[layer_idx] = None;
			// Return first view from next non-empty layer
			Some(self.first_view(base_layout))
		}
	}
}
