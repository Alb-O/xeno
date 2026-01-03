//! View focus management.
//!
//! Focusing buffers and navigating between views.

use xeno_base::Mode;
use xeno_registry::{HookContext, HookEventData, ViewId, emit_sync_with as emit_hook_sync_with};

use super::Editor;
use crate::buffer::{BufferId, BufferView};

/// Converts a buffer view to a hook-compatible view ID.
fn hook_view_id(view: BufferView) -> ViewId {
	ViewId::text(view.0)
}

impl Editor {
	/// Focuses a specific view explicitly (user action like click or keybinding).
	///
	/// Returns true if the view exists and was focused.
	/// Explicit focus can override sticky focus and will close dockables.
	pub fn focus_view(&mut self, view: BufferView) -> bool {
		self.focus_view_inner(view, true)
	}

	/// Focuses a specific view implicitly (mouse hover).
	///
	/// Returns true if the view exists and was focused.
	/// Respects sticky focus - won't steal focus from sticky views.
	pub fn focus_view_implicit(&mut self, view: BufferView) -> bool {
		let current = self.buffers.focused_view();
		if current == view || self.sticky_views.contains(&current) {
			return false;
		}
		self.focus_view_inner(view, false)
	}

	/// Internal focus implementation, handling sticky views.
	fn focus_view_inner(&mut self, view: BufferView, explicit: bool) -> bool {
		let old_view = self.buffers.focused_view();
		if !self.buffers.set_focused_view(view) {
			return false;
		}
		self.needs_redraw = true;

		if explicit && view != old_view {
			self.sticky_views.remove(&old_view);
		}

		if view != old_view {
			emit_hook_sync_with(
				&HookContext::new(
					HookEventData::ViewFocusChanged {
						view_id: hook_view_id(view),
						prev_view_id: Some(hook_view_id(old_view)),
					},
					Some(&self.extensions),
				),
				&mut self.hook_runtime,
			);
		}

		true
	}

	/// Focuses a specific buffer by ID.
	///
	/// Returns true if the buffer exists and was focused.
	pub fn focus_buffer(&mut self, id: BufferId) -> bool {
		self.focus_view(id)
	}

	/// Focuses the next view in the layout.
	pub fn focus_next_view(&mut self) {
		let next = self.layout.next_view(self.buffers.focused_view());
		self.focus_view(next);
	}

	/// Focuses the previous view in the layout.
	pub fn focus_prev_view(&mut self) {
		let prev = self.layout.prev_view(self.buffers.focused_view());
		self.focus_view(prev);
	}

	/// Focuses the next text buffer in the layout.
	pub fn focus_next_buffer(&mut self) {
		let current_id = self.buffers.focused_view();
		let next_id = self.layout.next_buffer(current_id);
		self.focus_buffer(next_id);
	}

	/// Focuses the previous text buffer in the layout.
	pub fn focus_prev_buffer(&mut self) {
		let current_id = self.buffers.focused_view();
		let prev_id = self.layout.prev_buffer(current_id);
		self.focus_buffer(prev_id);
	}

	/// Returns the current editing mode (Normal, Insert, Visual, etc.).
	pub fn mode(&self) -> Mode {
		self.buffer().input.mode()
	}

	/// Returns the display name for the current mode.
	pub fn mode_name(&self) -> &'static str {
		self.buffer().input.mode_name()
	}
}
