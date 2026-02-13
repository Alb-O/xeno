//! Layout navigation methods for traversing views and buffers.

use xeno_primitives::SpatialDirection;

use super::Layout;
use super::types::ViewId;
use crate::geometry::Rect;

impl Layout {
	/// Returns the next view in the layout order (for `Ctrl+w w` navigation).
	pub fn next_view(&self, current: ViewId) -> ViewId {
		let views = self.views();
		if views.is_empty() {
			return current;
		}
		let idx = views.iter().position(|&v| v == current).unwrap_or(0);
		views[(idx + 1) % views.len()]
	}

	/// Returns the previous view in the layout order.
	pub fn prev_view(&self, current: ViewId) -> ViewId {
		let views = self.views();
		if views.is_empty() {
			return current;
		}
		let idx = views.iter().position(|&v| v == current).unwrap_or(0);
		views[if idx == 0 { views.len() - 1 } else { idx - 1 }]
	}

	/// Returns the next buffer ID in layout order (for `:bnext`).
	pub fn next_buffer(&self, current: ViewId) -> ViewId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}
		let idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		ids[(idx + 1) % ids.len()]
	}

	/// Returns the previous buffer ID in layout order (for `:bprev`).
	pub fn prev_buffer(&self, current: ViewId) -> ViewId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}
		let idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		ids[if idx == 0 { ids.len() - 1 } else { idx - 1 }]
	}

	/// Finds the view in the given direction from the current view.
	///
	/// Candidates are scored by perpendicular overlap, edge distance, and
	/// proximity to `hint`. Wraps to the opposite edge if no view is found.
	pub fn view_in_direction(&self, area: Rect, current: ViewId, direction: SpatialDirection, hint: u16) -> Option<ViewId> {
		let views = self.compute_view_areas(area);
		let current_rect = views.iter().find(|(v, _)| *v == current)?.1;

		if let Some((v, _)) = views
			.iter()
			.filter(|(v, r)| *v != current && is_in_direction(current_rect, *r, direction))
			.max_by(|(_, a), (_, b)| compare_candidates(current_rect, *a, *b, direction, hint))
		{
			return Some(*v);
		}

		let wrap = opposite(direction);
		views
			.iter()
			.filter(|(v, r)| *v != current && is_in_direction(current_rect, *r, wrap))
			.max_by(|(_, a), (_, b)| compute_distance(current_rect, *a, wrap).cmp(&compute_distance(current_rect, *b, wrap)))
			.map(|(v, _)| *v)
	}
}

/// Returns the opposite direction.
fn opposite(direction: SpatialDirection) -> SpatialDirection {
	match direction {
		SpatialDirection::Left => SpatialDirection::Right,
		SpatialDirection::Right => SpatialDirection::Left,
		SpatialDirection::Up => SpatialDirection::Down,
		SpatialDirection::Down => SpatialDirection::Up,
	}
}

/// Checks if `candidate` is strictly in `direction` from `current`.
fn is_in_direction(current: Rect, candidate: Rect, direction: SpatialDirection) -> bool {
	match direction {
		SpatialDirection::Left => candidate.x + candidate.width <= current.x,
		SpatialDirection::Right => candidate.x >= current.x + current.width,
		SpatialDirection::Up => candidate.y + candidate.height <= current.y,
		SpatialDirection::Down => candidate.y >= current.y + current.height,
	}
}

/// Computes perpendicular overlap between two rects for a given direction.
///
/// For left/right movement, this is the vertical overlap.
/// For up/down movement, this is the horizontal overlap.
fn compute_overlap(current: Rect, candidate: Rect, direction: SpatialDirection) -> u16 {
	match direction {
		SpatialDirection::Left | SpatialDirection::Right => {
			let start = current.y.max(candidate.y);
			let end = (current.y + current.height).min(candidate.y + candidate.height);
			end.saturating_sub(start)
		}
		SpatialDirection::Up | SpatialDirection::Down => {
			let start = current.x.max(candidate.x);
			let end = (current.x + current.width).min(candidate.x + candidate.width);
			end.saturating_sub(start)
		}
	}
}

/// Computes edge distance between current and candidate in the given direction.
fn compute_distance(current: Rect, candidate: Rect, direction: SpatialDirection) -> u16 {
	match direction {
		SpatialDirection::Left => current.x.saturating_sub(candidate.x + candidate.width),
		SpatialDirection::Right => candidate.x.saturating_sub(current.x + current.width),
		SpatialDirection::Up => current.y.saturating_sub(candidate.y + candidate.height),
		SpatialDirection::Down => candidate.y.saturating_sub(current.y + current.height),
	}
}

/// Compares candidates by: overlap (more) > distance (less) > hint proximity (less).
fn compare_candidates(current: Rect, a: Rect, b: Rect, direction: SpatialDirection, hint: u16) -> std::cmp::Ordering {
	let overlap = compute_overlap(current, a, direction).cmp(&compute_overlap(current, b, direction));
	if overlap != std::cmp::Ordering::Equal {
		return overlap;
	}

	let dist = compute_distance(current, b, direction).cmp(&compute_distance(current, a, direction));
	if dist != std::cmp::Ordering::Equal {
		return dist;
	}

	let hint_dist = |r| (perpendicular_center(r, direction) as i32 - hint as i32).unsigned_abs();
	hint_dist(b).cmp(&hint_dist(a))
}

/// Returns the center position along the perpendicular axis.
fn perpendicular_center(rect: Rect, direction: SpatialDirection) -> u16 {
	match direction {
		SpatialDirection::Left | SpatialDirection::Right => rect.y + rect.height / 2,
		SpatialDirection::Up | SpatialDirection::Down => rect.x + rect.width / 2,
	}
}
