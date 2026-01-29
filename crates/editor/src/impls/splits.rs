//! Split and view management.
//!
//! Creating, closing, and managing split views.

use std::path::PathBuf;

use xeno_registry::{
	HookContext, HookEventData, SplitDirection, ViewId, emit_sync_with as emit_hook_sync_with,
};

use super::Editor;

impl Editor {
	/// Creates a new buffer that shares the same document as the focused buffer.
	fn clone_focused_buffer_for_split(&mut self) -> ViewId {
		let _focused_id = self.focused_view();
		let new_id = ViewId(self.state.core.buffers.next_buffer_id());

		let new_buffer = self.buffer().clone_for_split(new_id);
		let _doc_id = new_buffer.document_id();

		self.state.core.buffers.insert_buffer(new_id, new_buffer);
		new_id
	}

	/// Creates a horizontal split with the current view and a new buffer below.
	///
	/// Matches Vim's `:split` / Helix's `hsplit` (Ctrl+w s).
	pub fn split_horizontal_with_clone(&mut self) {
		let new_id = self.clone_focused_buffer_for_split();
		self.split_horizontal(new_id);
	}

	/// Creates a vertical split with the current view and a new buffer to the right.
	///
	/// Matches Vim's `:vsplit` / Helix's `vsplit` (Ctrl+w v).
	pub fn split_vertical_with_clone(&mut self) {
		let new_id = self.clone_focused_buffer_for_split();
		self.split_vertical(new_id);
	}

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
			&HookContext::new(HookEventData::SplitCreated {
				view_id: new_buffer_id,
				direction: SplitDirection::Horizontal,
			}),
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
			&HookContext::new(HookEventData::SplitCreated {
				view_id: new_buffer_id,
				direction: SplitDirection::Vertical,
			}),
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

		if let Some(buffer) = self.state.core.buffers.get_buffer(view) {
			let scratch_path = PathBuf::from("[scratch]");
			let path = buffer.path().unwrap_or_else(|| scratch_path.clone());
			let file_type = buffer.file_type();
			emit_hook_sync_with(
				&HookContext::new(HookEventData::BufferClose {
					path: &path,
					file_type: file_type.as_deref(),
				}),
				&mut self.state.hook_runtime,
			);

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.lsp.on_buffer_close(buffer) {
				tracing::warn!(error = %e, "LSP buffer close failed");
			}
		}

		emit_hook_sync_with(
			&HookContext::new(HookEventData::SplitClosed { view_id: view }),
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

		self.finalize_buffer_removal(view);

		self.repair_invariants();

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
