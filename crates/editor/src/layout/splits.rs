//! Split creation and removal.
//!
//! Creating horizontal/vertical splits and removing views from the layout.

use super::manager::LayoutManager;
use super::types::LayerId;
use crate::buffer::{Layout, ViewId};
use crate::geometry::Rect;

/// Errors that can occur during split operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitError {
	/// The target view was not found in any layout layer.
	ViewNotFound,
	/// The view area is too small to split (below minimum size requirements).
	AreaTooSmall,
}

impl LayoutManager {
	/// Determines if a horizontal split can be created for the given view.
	///
	/// Returns the layer ID and view area if the split is feasible, or an error
	/// describing why it cannot proceed.
	///
	/// # Errors
	///
	/// - [`SplitError::ViewNotFound`] if the view is not found in any layer.
	/// - [`SplitError::AreaTooSmall`] if the view area height is less than 3.
	pub fn can_split_horizontal(&self, base_layout: &Layout, current_view: ViewId, doc_area: Rect) -> Result<(LayerId, Rect), SplitError> {
		let layer = self.layer_of_view(base_layout, current_view).ok_or(SplitError::ViewNotFound)?;

		let view_area = self.view_area(base_layout, current_view, doc_area).ok_or(SplitError::ViewNotFound)?;

		if view_area.height < 3 {
			return Err(SplitError::AreaTooSmall);
		}

		Ok((layer, view_area))
	}

	/// Determines if a vertical split can be created for the given view.
	///
	/// Returns the layer ID and view area if the split is feasible, or an error
	/// describing why it cannot proceed.
	///
	/// # Errors
	///
	/// - [`SplitError::ViewNotFound`] if the view is not found in any layer.
	/// - [`SplitError::AreaTooSmall`] if the view area width is less than 3.
	pub fn can_split_vertical(&self, base_layout: &Layout, current_view: ViewId, doc_area: Rect) -> Result<(LayerId, Rect), SplitError> {
		let layer = self.layer_of_view(base_layout, current_view).ok_or(SplitError::ViewNotFound)?;

		let view_area = self.view_area(base_layout, current_view, doc_area).ok_or(SplitError::ViewNotFound)?;

		if view_area.width < 3 {
			return Err(SplitError::AreaTooSmall);
		}

		Ok((layer, view_area))
	}

	/// Creates a horizontal split with a new buffer below the current view.
	///
	/// # Panics
	///
	/// Panics if the view is not found or its area cannot be computed.
	/// The caller must call [`Self::can_split_horizontal`] first.
	pub fn split_horizontal(&mut self, base_layout: &mut Layout, current_view: ViewId, new_buffer_id: ViewId, doc_area: Rect) {
		let layer = self.layer_of_view(base_layout, current_view).expect("view must exist (preflight required)");
		let view_area = self
			.view_area(base_layout, current_view, doc_area)
			.expect("view must have area (preflight required)");

		let new_layout = Layout::stacked(Layout::single(current_view), Layout::text(new_buffer_id), view_area);

		if layer.is_base() {
			base_layout.replace_view(current_view, new_layout);
		} else {
			let layout = self.layer_mut(base_layout, layer).expect("overlay layer must exist (preflight required)");
			layout.replace_view(current_view, new_layout);
		}
		self.increment_revision();
	}

	/// Creates a vertical split with a new buffer to the right of the current view.
	///
	/// # Panics
	///
	/// Panics if the view is not found or its area cannot be computed.
	/// The caller must call [`Self::can_split_vertical`] first.
	pub fn split_vertical(&mut self, base_layout: &mut Layout, current_view: ViewId, new_buffer_id: ViewId, doc_area: Rect) {
		let layer = self.layer_of_view(base_layout, current_view).expect("view must exist (preflight required)");
		let view_area = self
			.view_area(base_layout, current_view, doc_area)
			.expect("view must have area (preflight required)");

		let new_layout = Layout::side_by_side(Layout::single(current_view), Layout::text(new_buffer_id), view_area);

		if layer.is_base() {
			base_layout.replace_view(current_view, new_layout);
		} else {
			let layout = self.layer_mut(base_layout, layer).expect("overlay layer must exist (preflight required)");
			layout.replace_view(current_view, new_layout);
		}
		self.increment_revision();
	}

	/// Gets the area of a specific view.
	pub(super) fn view_area(&self, base_layout: &Layout, view: ViewId, doc_area: Rect) -> Option<Rect> {
		let layer = self.layer_of_view(base_layout, view)?;
		let layer_area = self.layer_area(layer, doc_area);
		self.layer(base_layout, layer)
			.ok()?
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
	///
	/// # Errors
	///
	/// Returns `None` if:
	/// - The view is not found in any layer.
	/// - The view is the last one in the base layer (closing denied).
	/// - Structural removal fails internally.
	pub fn remove_view(&mut self, base_layout: &mut Layout, view: ViewId, doc_area: Rect) -> Option<ViewId> {
		let layer = self.layer_of_view(base_layout, view)?;

		if layer.is_base() && base_layout.count() <= 1 {
			return None;
		}

		let layer_area = self.layer_area(layer, doc_area);

		let before = if layer.is_base() {
			base_layout.compute_view_areas(layer_area)
		} else {
			let idx = self.validate_layer(layer).ok()?;
			self.layers[idx].layout.as_ref()?.compute_view_areas(layer_area)
		};

		if layer.is_base() {
			*base_layout = base_layout.remove_view(view)?;
			self.increment_revision();
			let after = base_layout.compute_view_areas(layer_area);
			return suggested_focus_after_close(&before, &after, view).or_else(|| Some(base_layout.first_view()));
		}

		let idx = self.validate_layer(layer).ok()?;

		let cleared = {
			let slot = &mut self.layers[idx];
			let new_layout_opt = slot.layout.as_ref()?.remove_view(view);
			if let Some(new_layout) = new_layout_opt {
				slot.layout = Some(new_layout);
				false
			} else {
				slot.layout = None;
				slot.generation = slot.generation.wrapping_add(1);
				true
			}
		};

		self.increment_revision();

		if cleared {
			Some(self.first_view(base_layout))
		} else {
			let after = self.layers[idx].layout.as_ref().unwrap().compute_view_areas(layer_area);
			suggested_focus_after_close(&before, &after, view).or_else(|| Some(self.layers[idx].layout.as_ref().unwrap().first_view()))
		}
	}
}

/// Finds the best view to focus after closing a view using spatial overlap.
///
/// Prefers the view with the most overlap with the closed view's area (i.e., the
/// view that expanded to fill the hole). On ties, prefers views closer to the
/// closed view's center.
fn suggested_focus_after_close(before: &[(ViewId, Rect)], after: &[(ViewId, Rect)], closed: ViewId) -> Option<ViewId> {
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
	let x1 = a.right().min(b.right());
	let y1 = a.bottom().min(b.bottom());
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
