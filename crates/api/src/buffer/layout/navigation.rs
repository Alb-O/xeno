//! Layout navigation methods for traversing views and buffers.

use super::types::BufferView;
use super::Layout;
use crate::buffer::BufferId;

impl Layout {
	/// Returns the next view in the layout order (for `Ctrl+w w` navigation).
	pub fn next_view(&self, current: BufferView) -> BufferView {
		let views = self.views();
		if views.is_empty() {
			return current;
		}
		let idx = views.iter().position(|&v| v == current).unwrap_or(0);
		views[(idx + 1) % views.len()]
	}

	/// Returns the previous view in the layout order.
	pub fn prev_view(&self, current: BufferView) -> BufferView {
		let views = self.views();
		if views.is_empty() {
			return current;
		}
		let idx = views.iter().position(|&v| v == current).unwrap_or(0);
		views[if idx == 0 { views.len() - 1 } else { idx - 1 }]
	}

	/// Returns the next buffer ID in layout order (for `:bnext`).
	pub fn next_buffer(&self, current: BufferId) -> BufferId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}
		let idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		ids[(idx + 1) % ids.len()]
	}

	/// Returns the previous buffer ID in layout order (for `:bprev`).
	pub fn prev_buffer(&self, current: BufferId) -> BufferId {
		let ids = self.buffer_ids();
		if ids.is_empty() {
			return current;
		}
		let idx = ids.iter().position(|&id| id == current).unwrap_or(0);
		ids[if idx == 0 { ids.len() - 1 } else { idx - 1 }]
	}
}
