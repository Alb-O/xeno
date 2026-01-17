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
			self.increment_revision();
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
			self.increment_revision();
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
	/// Returns the suggested view to focus after removal. Uses spatial overlap
	/// scoring to find the view that expanded into the closed view's space,
	/// providing a more intuitive focus transition than always picking the first view.
	pub fn remove_view(
		&mut self,
		base_layout: &mut Layout,
		view: BufferView,
		doc_area: Rect,
	) -> Option<BufferView> {
		let layer_idx = self.layer_of_view(base_layout, view)?;

		if layer_idx == 0 && base_layout.count() <= 1 {
			return None;
		}

		let layer_area = self.layer_area(layer_idx, doc_area);
		let before = if layer_idx == 0 {
			base_layout.compute_view_areas(layer_area)
		} else {
			self.layers[layer_idx]
				.as_ref()?
				.compute_view_areas(layer_area)
		};

		if layer_idx == 0 {
			*base_layout = base_layout.remove_view(view)?;
			self.increment_revision();
			let after = base_layout.compute_view_areas(layer_area);
			return suggested_focus_after_close(&before, &after, view)
				.or_else(|| Some(base_layout.first_view()));
		}

		let new_layout = self.layers[layer_idx].as_ref()?.remove_view(view);

		if let Some(new_layout) = new_layout {
			self.layers[layer_idx] = Some(new_layout);
			self.increment_revision();
			let after = self.layers[layer_idx]
				.as_ref()
				.unwrap()
				.compute_view_areas(layer_area);
			suggested_focus_after_close(&before, &after, view)
				.or_else(|| Some(self.layers[layer_idx].as_ref().unwrap().first_view()))
		} else {
			self.layers[layer_idx] = None;
			self.increment_revision();
			Some(self.first_view(base_layout))
		}
	}
}

/// Finds the best view to focus after closing a view using spatial overlap.
///
/// Prefers the view with the most overlap with the closed view's area (i.e., the
/// view that expanded to fill the hole). On ties, prefers views closer to the
/// closed view's center.
fn suggested_focus_after_close(
	before: &[(BufferView, Rect)],
	after: &[(BufferView, Rect)],
	closed: BufferView,
) -> Option<BufferView> {
	let closed_rect = before.iter().find(|(v, _)| *v == closed)?.1;
	after
		.iter()
		.map(|(v, r)| (*v, overlap_area(closed_rect, *r), center_dist_sq(closed_rect, *r)))
		.max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)))
		.map(|(v, _, _)| v)
}

/// Computes the area of overlap between two rectangles.
fn overlap_area(a: Rect, b: Rect) -> u32 {
	let x0 = a.x.max(b.x);
	let y0 = a.y.max(b.y);
	let x1 = (a.x + a.width).min(b.x + b.width);
	let y1 = (a.y + a.height).min(b.y + b.height);
	let w = x1.saturating_sub(x0) as u32;
	let h = y1.saturating_sub(y0) as u32;
	w * h
}

/// Computes the squared distance between the centers of two rectangles.
fn center_dist_sq(a: Rect, b: Rect) -> u32 {
	let ax = a.x as i32 + a.width as i32 / 2;
	let ay = a.y as i32 + a.height as i32 / 2;
	let bx = b.x as i32 + b.width as i32 / 2;
	let by = b.y as i32 + b.height as i32 / 2;
	let dx = ax - bx;
	let dy = ay - by;
	(dx * dx + dy * dy) as u32
}
