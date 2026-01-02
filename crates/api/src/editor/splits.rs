//! Split and view management.
//!
//! Creating, closing, and managing split views.

use std::path::PathBuf;

use evildoer_registry::panels::{PanelId, find_panel, panel_kind_index};
use evildoer_registry::{
	HookContext, HookEventData, SplitDirection, ViewId, emit_sync_with as emit_hook_sync_with,
};

use super::Editor;
use crate::buffer::{BufferId, BufferView, Layout};

/// Converts a buffer view to a hook-compatible view ID.
fn hook_view_id(view: BufferView) -> ViewId {
	match view {
		BufferView::Text(id) => ViewId::Text(id.0),
		BufferView::Panel(id) => ViewId::Panel(id),
	}
}

impl Editor {
	/// Layer index for the docked terminal panel.
	pub(super) const DOCK_LAYER: usize = 1;

	/// Creates a horizontal split with the current view and a new buffer below.
	///
	/// Matches Vim's `:split` / Helix's `hsplit` (Ctrl+w s).
	pub fn split_horizontal(&mut self, new_buffer_id: BufferId) {
		let current_view = self.buffers.focused_view();
		let doc_area = self.doc_area();
		self.layout
			.split_horizontal(current_view, new_buffer_id, doc_area);
		self.focus_buffer(new_buffer_id);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::SplitCreated {
					view_id: hook_view_id(BufferView::Text(new_buffer_id)),
					direction: SplitDirection::Horizontal,
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
	}

	/// Creates a vertical split with the current view and a new buffer to the right.
	///
	/// Matches Vim's `:vsplit` / Helix's `vsplit` (Ctrl+w v).
	pub fn split_vertical(&mut self, new_buffer_id: BufferId) {
		let current_view = self.buffers.focused_view();
		let doc_area = self.doc_area();
		self.layout
			.split_vertical(current_view, new_buffer_id, doc_area);
		self.focus_buffer(new_buffer_id);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::SplitCreated {
					view_id: hook_view_id(BufferView::Text(new_buffer_id)),
					direction: SplitDirection::Vertical,
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
	}

	/// Toggles a panel by name.
	///
	/// If the panel is visible, hides it. Otherwise shows it on its configured layer.
	pub fn toggle_panel(&mut self, name: &str) -> bool {
		let Some(def) = find_panel(name) else {
			self.notify("error", format!("Unknown panel: {}", name));
			return false;
		};
		let Some(kind) = panel_kind_index(name) else {
			return false;
		};

		if let Some(panel_id) = self.panels.find_by_kind(kind) {
			let view = BufferView::Panel(panel_id);
			if self.layout.contains_view(view) {
				self.sticky_views.remove(&view);
				self.layout.set_layer(def.layer, None);
				self.buffers.set_focused_view(self.layout.first_view());
				self.needs_redraw = true;
				emit_hook_sync_with(
					&HookContext::new(
						HookEventData::PanelToggled {
							panel_id: def.id,
							visible: false,
						},
						Some(&self.extensions),
					),
					&mut self.hook_runtime,
				);
				return true;
			}
		}

		let Some(panel_id) = self.panels.get_or_create(name) else {
			self.notify("error", format!("Failed to create panel: {}", name));
			return false;
		};

		let panel_view = BufferView::Panel(panel_id);
		if def.sticky {
			self.sticky_views.insert(panel_view);
		}
		self.layout
			.set_layer(def.layer, Some(Layout::single(panel_view)));
		self.buffers.set_focused_view(panel_view);
		self.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::PanelToggled {
					panel_id: def.id,
					visible: true,
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
		true
	}

	/// Requests the editor to quit after the current event loop iteration.
	pub fn request_quit(&mut self) {
		self.pending_quit = true;
	}

	/// Consumes and returns the pending quit request, if any.
	pub fn take_quit_request(&mut self) -> bool {
		if self.pending_quit {
			self.pending_quit = false;
			true
		} else {
			false
		}
	}

	/// Closes a view (buffer or panel).
	///
	/// Returns true if the view was closed.
	pub fn close_view(&mut self, view: BufferView) -> bool {
		if self.layout.count() <= 1 {
			return false;
		}

		if let BufferView::Text(id) = view
			&& let Some(buffer) = self.buffers.get_buffer(id)
		{
			let scratch_path = PathBuf::from("[scratch]");
			let path = buffer.path().unwrap_or_else(|| scratch_path.clone());
			let file_type = buffer.file_type();
			emit_hook_sync_with(
				&HookContext::new(
					HookEventData::BufferClose {
						path: &path,
						file_type: file_type.as_deref(),
					},
					Some(&self.extensions),
				),
				&mut self.hook_runtime,
			);
		}

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::SplitClosed {
					view_id: hook_view_id(view),
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);

		// Remove from layout - returns the new focus target if successful
		let new_focus = self.layout.remove_view(view);
		if new_focus.is_none() {
			return false;
		}

		match view {
			BufferView::Text(id) => {
				self.buffers.remove_buffer(id);
			}
			BufferView::Panel(id) => {
				self.panels.remove(id);
			}
		}

		// If we closed the focused view, focus another one
		if self.buffers.focused_view() == view
			&& let Some(focus) = new_focus
		{
			self.buffers.set_focused_view(focus);
		}

		self.needs_redraw = true;
		true
	}

	/// Closes a buffer.
	///
	/// Returns true if the buffer was closed.
	pub fn close_buffer(&mut self, id: BufferId) -> bool {
		self.close_view(BufferView::Text(id))
	}

	/// Closes a panel.
	///
	/// Returns true if the panel was closed.
	pub fn close_panel(&mut self, id: PanelId) -> bool {
		self.close_view(BufferView::Panel(id))
	}

	/// Closes the current view (buffer or panel).
	///
	/// Returns true if the view was closed.
	pub fn close_current_view(&mut self) -> bool {
		self.close_view(self.buffers.focused_view())
	}

	/// Closes the current buffer if a text buffer is focused.
	///
	/// Returns true if the buffer was closed.
	pub fn close_current_buffer(&mut self) -> bool {
		match self.buffers.focused_view() {
			BufferView::Text(id) => self.close_buffer(id),
			BufferView::Panel(_) => false,
		}
	}
}
