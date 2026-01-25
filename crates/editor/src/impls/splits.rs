//! Split and view management.
//!
//! Creating, closing, and managing split views.

use std::path::PathBuf;

#[cfg(feature = "lsp")]
use tracing::warn;
use xeno_registry::{
	HookContext, HookEventData, SplitDirection, ViewId, emit_sync_with as emit_hook_sync_with,
};

use super::Editor;

impl Editor {
	/// Creates a horizontal split with the current view and a new buffer below.
	///
	/// Matches Vim's `:split` / Helix's `hsplit` (Ctrl+w s).
	pub fn split_horizontal(&mut self, new_buffer_id: ViewId) {
		let current_view = self.focused_view();
		let doc_area = self.doc_area();
		let base_layout = &mut self.state.windows.base_window_mut().layout;
		let layout = &mut self.state.layout;
		layout.split_horizontal(base_layout, current_view, new_buffer_id, doc_area);
		self.focus_buffer(new_buffer_id);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::SplitCreated {
					view_id: new_buffer_id,
					direction: SplitDirection::Horizontal,
				},
				Some(&self.state.extensions),
			),
			&mut self.state.hook_runtime,
		);
	}

	/// Creates a vertical split with the current view and a new buffer to the right.
	///
	/// Matches Vim's `:vsplit` / Helix's `vsplit` (Ctrl+w v).
	pub fn split_vertical(&mut self, new_buffer_id: ViewId) {
		let current_view = self.focused_view();
		let doc_area = self.doc_area();
		let base_layout = &mut self.state.windows.base_window_mut().layout;
		let layout = &mut self.state.layout;
		layout.split_vertical(base_layout, current_view, new_buffer_id, doc_area);
		self.focus_buffer(new_buffer_id);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::SplitCreated {
					view_id: new_buffer_id,
					direction: SplitDirection::Vertical,
				},
				Some(&self.state.extensions),
			),
			&mut self.state.hook_runtime,
		);
	}

	/// Requests the editor to quit after the current event loop iteration.
	pub fn request_quit(&mut self) {
		self.state.frame.pending_quit = true;
	}

	/// Consumes and returns the pending quit request, if any.
	pub fn take_quit_request(&mut self) -> bool {
		if self.state.frame.pending_quit {
			self.state.frame.pending_quit = false;
			true
		} else {
			false
		}
	}

	/// Closes a view (buffer).
	///
	/// Returns true if the view was closed.
	pub fn close_view(&mut self, view: ViewId) -> bool {
		if self.state.layout.count(&self.base_window().layout) <= 1 {
			return false;
		}

		#[cfg(feature = "lsp")]
		let doc_id_to_close = self
			.state
			.core
			.buffers
			.get_buffer(view)
			.map(|b| b.document_id());

		if let Some(buffer) = self.state.core.buffers.get_buffer(view) {
			let scratch_path = PathBuf::from("[scratch]");
			let path = buffer.path().unwrap_or_else(|| scratch_path.clone());
			let file_type = buffer.file_type();
			emit_hook_sync_with(
				&HookContext::new(
					HookEventData::BufferClose {
						path: &path,
						file_type: file_type.as_deref(),
					},
					Some(&self.state.extensions),
				),
				&mut self.state.hook_runtime,
			);

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.lsp.on_buffer_close(buffer) {
				warn!(error = %e, "LSP buffer close failed");
			}
		}

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::SplitClosed { view_id: view },
				Some(&self.state.extensions),
			),
			&mut self.state.hook_runtime,
		);

		// Remove from layout - returns the new focus target if successful
		let doc_area = self.doc_area();
		let base_layout = &mut self.state.windows.base_window_mut().layout;
		let layout = &mut self.state.layout;
		let new_focus = layout.remove_view(base_layout, view, doc_area);
		if new_focus.is_none() {
			return false;
		}

		self.state.core.buffers.remove_buffer(view);

		// Close in sync manager if this was the last view for the document
		#[cfg(feature = "lsp")]
		if let Some(doc_id) = doc_id_to_close
			&& self.state.core.buffers.any_buffer_for_doc(doc_id).is_none()
		{
			self.state.lsp.sync_manager_mut().on_doc_close(doc_id);
		}

		// If we closed the focused view, focus another one
		if self.focused_view() == view
			&& let Some(focus) = new_focus
		{
			self.focus_view(focus);
		}

		self.state.frame.needs_redraw = true;
		true
	}

	/// Closes a buffer.
	///
	/// Returns true if the buffer was closed.
	pub fn close_buffer(&mut self, id: ViewId) -> bool {
		self.close_view(id)
	}

	/// Closes the current view (buffer).
	///
	/// Returns true if the view was closed.
	pub fn close_current_view(&mut self) -> bool {
		self.close_view(self.focused_view())
	}

	/// Closes the current buffer if a text buffer is focused.
	///
	/// Returns true if the buffer was closed.
	pub fn close_current_buffer(&mut self) -> bool {
		self.close_buffer(self.focused_view())
	}
}
