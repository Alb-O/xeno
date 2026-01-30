//! Split and view management.
//!
//! Creating, closing, and managing split views.

use std::path::PathBuf;

use xeno_registry::{
	HookContext, HookEventData, SplitDirection, ViewId, emit_sync_with as emit_hook_sync_with,
};

use super::Editor;
use crate::layout::SplitError;

impl Editor {
	/// Allocates a new buffer for a split operation.
	///
	/// The caller is responsible for ensuring the split will succeed before calling this.
	fn allocate_split_buffer(&mut self) -> ViewId {
		let new_id = ViewId(self.state.core.buffers.next_buffer_id());
		let new_buffer = self.buffer().clone_for_split(new_id);
		self.state.core.buffers.insert_buffer(new_id, new_buffer);
		new_id
	}

	/// Creates a horizontal split with the current view and a new buffer below.
	///
	/// Matches Vim's `:split` / Helix's `hsplit` (`Ctrl+w s`).
	///
	/// This operation is atomic: if the split cannot be created (e.g., view not found
	/// or area too small), no buffer is allocated and no state changes occur.
	///
	/// # Errors
	///
	/// Returns [`SplitError`] if the preflight check fails.
	pub fn split_horizontal_with_clone(&mut self) -> Result<(), SplitError> {
		let current_view = self.focused_view();
		let doc_area = self.doc_area();
		let base_layout = &self.state.windows.base_window().layout;

		let (_layer, _view_area) =
			self.state
				.layout
				.can_split_horizontal(base_layout, current_view, doc_area)?;

		let new_id = self.allocate_split_buffer();

		let base_layout = &mut self.state.windows.base_window_mut().layout;
		let layout = &mut self.state.layout;
		layout.split_horizontal(base_layout, current_view, new_id, doc_area);

		self.focus_buffer(new_id);
		emit_hook_sync_with(
			&HookContext::new(HookEventData::SplitCreated {
				view_id: new_id,
				direction: SplitDirection::Horizontal,
			}),
			&mut self.state.hook_runtime,
		);

		Ok(())
	}

	/// Creates a vertical split with the current view and a new buffer to the right.
	///
	/// Matches Vim's `:vsplit` / Helix's `vsplit` (`Ctrl+w v`).
	///
	/// This operation is atomic: if the split cannot be created (e.g., view not found
	/// or area too small), no buffer is allocated and no state changes occur.
	///
	/// # Errors
	///
	/// Returns [`SplitError`] if the preflight check fails.
	pub fn split_vertical_with_clone(&mut self) -> Result<(), SplitError> {
		let current_view = self.focused_view();
		let doc_area = self.doc_area();
		let base_layout = &self.state.windows.base_window().layout;

		let (_layer, _view_area) =
			self.state
				.layout
				.can_split_vertical(base_layout, current_view, doc_area)?;

		let new_id = self.allocate_split_buffer();

		let base_layout = &mut self.state.windows.base_window_mut().layout;
		let layout = &mut self.state.layout;
		layout.split_vertical(base_layout, current_view, new_id, doc_area);

		self.focus_buffer(new_id);
		emit_hook_sync_with(
			&HookContext::new(HookEventData::SplitCreated {
				view_id: new_id,
				direction: SplitDirection::Vertical,
			}),
			&mut self.state.hook_runtime,
		);

		Ok(())
	}

	/// Creates a horizontal split with an existing buffer.
	///
	/// # Panics
	///
	/// Panics if the split cannot be applied. Prefer [`Self::split_horizontal_with_clone`]
	/// for atomicity.
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

	/// Creates a vertical split with an existing buffer.
	///
	/// # Panics
	///
	/// Panics if the split cannot be applied. Prefer [`Self::split_vertical_with_clone`]
	/// for atomicity.
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

	/// Closes a specific view and cleans up the associated buffer.
	///
	/// Returns `true` if the view was closed. The operation is ordered to ensure
	/// consistency:
	/// 1. Verify the view exists and can be removed.
	/// 2. Remove from layout and determine suggested focus.
	/// 3. Emit hooks only after successful removal.
	/// 4. Update focus.
	/// 5. Clean up the buffer store.
	pub fn close_view(&mut self, view: ViewId) -> bool {
		let doc_area = self.doc_area();
		let base_layout = &self.state.windows.base_window().layout;

		let layer = match self.state.layout.layer_of_view(base_layout, view) {
			Some(id) => id,
			None => return false,
		};

		if layer.is_base() && base_layout.count() <= 1 {
			return false;
		}

		let focused_view = self.focused_view();
		let was_focused = focused_view == view;

		let base_layout = &mut self.state.windows.base_window_mut().layout;
		let layout = &mut self.state.layout;
		let new_focus = match layout.remove_view(base_layout, view, doc_area) {
			Some(focus) => focus,
			None => return false,
		};

		let current_focus_still_valid =
			layout.contains_view(base_layout, focused_view) && focused_view != view;
		if was_focused || !current_focus_still_valid {
			self.focus_buffer(new_focus);
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

		self.finalize_buffer_removal(view);
		self.repair_invariants();

		self.state.frame.needs_redraw = true;
		true
	}

	/// Closes a buffer.
	pub fn close_buffer(&mut self, id: ViewId) -> bool {
		self.close_view(id)
	}

	/// Closes the current view.
	pub fn close_current_view(&mut self) -> bool {
		self.close_view(self.focused_view())
	}

	/// Closes the current buffer if a text buffer is focused.
	pub fn close_current_buffer(&mut self) -> bool {
		self.close_buffer(self.focused_view())
	}
}
