//! File operations (save, load).

use std::io;
use std::path::{Path, PathBuf};

use ropey::Rope;
#[cfg(feature = "lsp")]
use tracing::warn;
use xeno_primitives::BoxFutureLocal;
use xeno_registry::HookEventData;
use xeno_registry::commands::CommandError;
use xeno_registry::hooks::{HookContext, emit as emit_hook};

use super::Editor;

impl Editor {
	/// Returns true if the current buffer has unsaved changes.
	pub fn is_modified(&self) -> bool {
		self.buffer().modified()
	}

	/// Saves the current buffer to its file path.
	pub fn save(&mut self) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			let path_owned = match &self.buffer().path() {
				Some(p) => p.clone(),
				None => {
					return Err(CommandError::InvalidArgument("No filename. Use :write <filename>".to_string()));
				}
			};

			// Snapshot content once to minimize lock hold time and avoid double cloning.
			let rope = self.buffer().with_doc(|doc| doc.content().clone());

			emit_hook(&HookContext::new(HookEventData::BufferWritePre {
				path: &path_owned,
				text: rope.slice(..),
			}))
			.await;

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.lsp.on_buffer_will_save(self.buffer()).await {
				warn!(error = %e, "LSP will_save notification failed");
			}

			// Encode content without holding the document lock.
			let content = {
				let mut content = Vec::with_capacity(rope.len_bytes());
				for chunk in rope.chunks() {
					content.extend_from_slice(chunk.as_bytes());
				}
				content
			};

			if let Some(parent) = path_owned.parent()
				&& !parent.as_os_str().is_empty()
			{
				tokio::fs::create_dir_all(parent).await.map_err(|e| CommandError::Io(e.to_string()))?;
			}

			tokio::fs::write(&path_owned, &content).await.map_err(|e| CommandError::Io(e.to_string()))?;

			let _ = self.buffer_mut().set_modified(false);
			self.show_notification(xeno_registry::notifications::keys::file_saved(&path_owned));

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.lsp.on_buffer_did_save(self.buffer(), true).await {
				warn!(error = %e, "LSP did_save notification failed");
			}

			emit_hook(&HookContext::new(HookEventData::BufferWrite { path: &path_owned })).await;

			Ok(())
		})
	}

	/// Saves the current buffer to a new file path.
	pub fn save_as(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		let loader_arc = self.state.config.language_loader.clone();
		let _ = self.buffer_mut().set_path(Some(path), Some(&loader_arc));
		self.save()
	}

	/// Applies a loaded file to the editor.
	///
	/// Called by [`crate::msg::IoMsg::FileLoaded`] when background file loading completes.
	/// Token-gated: ignores stale loads (superseded by a newer request). Also
	/// refuses to overwrite a buffer that has been modified since the load was
	/// kicked, preserving user edits.
	pub(crate) fn apply_loaded_file(&mut self, path: PathBuf, rope: Rope, readonly: bool, token: u64) {
		// Stale token check: only apply if this token matches the pending load for this path.
		let is_current = self.state.pending_file_loads.get(&path) == Some(&token);
		if !is_current {
			tracing::debug!(path = %path.display(), token, "Ignoring stale file load");
			return;
		}

		self.state.pending_file_loads.remove(&path);

		let Some(buffer_id) = self.state.core.buffers.find_by_path(&path) else {
			tracing::warn!(path = %path.display(), "No buffer found for loaded file");
			return;
		};

		let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) else {
			return;
		};

		// Don't clobber user edits that happened while the load was in flight.
		if buffer.modified() {
			tracing::warn!(path = %path.display(), "Buffer modified during load; preserving user edits");
			self.show_notification(xeno_registry::notifications::keys::warn(format!(
				"File load for {} arrived after edits; keeping current content",
				path.display()
			)));
			return;
		}

		tracing::debug!(path = %path.display(), len = rope.len_bytes(), "File loaded");

		buffer.reset_content(rope.clone());
		self.state.syntax_manager.reset_syntax(buffer.document_id());
		if readonly {
			buffer.set_readonly(true);
		}
		buffer.set_modified(false);
		buffer.set_cursor_and_selection(0, xeno_primitives::Selection::point(0));

		let file_type = buffer.file_type();

		#[cfg(feature = "lsp")]
		self.maybe_track_lsp_for_buffer(buffer_id, true);

		crate::impls::emit_hook_sync_with(
			&HookContext::new(HookEventData::BufferOpen {
				path: &path,
				text: rope.slice(..),
				file_type: file_type.as_deref(),
			}),
			&mut self.state.work_scheduler,
		);

		if let Some((line, column)) = self.state.deferred_goto.take() {
			self.goto_line_col(line, column);
		}
	}

	/// Notifies the user of a file load error and clears loading state.
	pub(crate) fn notify_load_error(&mut self, path: &Path, error: &io::Error, token: u64) {
		if self.state.pending_file_loads.get(path) == Some(&token) {
			self.state.pending_file_loads.remove(path);
		}
		self.show_notification(xeno_registry::notifications::keys::error(format!(
			"Failed to load {}: {}",
			path.display(),
			error
		)));
	}

	/// Returns true if the given path has a pending background load.
	pub fn is_loading_file(&self, path: &Path) -> bool {
		self.state.pending_file_loads.contains_key(path)
	}

	/// Returns true if any file is currently being loaded in the background.
	pub fn has_pending_file_loads(&self) -> bool {
		!self.state.pending_file_loads.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use ropey::Rope;

	use crate::Editor;

	#[tokio::test]
	async fn file_loaded_stale_token_is_ignored() {
		let mut editor = Editor::new_scratch();
		let path = PathBuf::from("/tmp/test_stale.txt");

		// Create a buffer for the path and track its view ID.
		let view_id = editor.open_file(path.clone()).await.expect("open file");

		// Simulate pending load with token=2 (the "current" request).
		editor.state.pending_file_loads.insert(path.clone(), 2);

		// Apply a stale load (token=1) — should be ignored.
		let stale_rope = Rope::from_str("stale content");
		editor.apply_loaded_file(path.clone(), stale_rope, false, 1);

		let buf = editor.state.core.buffers.get_buffer(view_id).unwrap();
		let content = buf.with_doc(|doc| doc.content().to_string());
		assert_ne!(content, "stale content", "stale token should not overwrite buffer");

		// Pending load should still be active (not cleared by the stale token).
		assert!(
			editor.state.pending_file_loads.contains_key(&path),
			"pending load should remain for the current token"
		);

		// Apply the current load (token=2) — should succeed.
		let current_rope = Rope::from_str("current content");
		editor.apply_loaded_file(path.clone(), current_rope, false, 2);

		let buf = editor.state.core.buffers.get_buffer(view_id).unwrap();
		let content = buf.with_doc(|doc| doc.content().to_string());
		assert_eq!(content, "current content", "current token should replace buffer content");
		assert!(!editor.state.pending_file_loads.contains_key(&path), "pending load should be cleared");
	}

	#[tokio::test]
	async fn file_loaded_does_not_clobber_modified_buffer() {
		let mut editor = Editor::new_scratch();
		let path = PathBuf::from("/tmp/test_modified.txt");

		// Create a buffer for the path and track its view ID.
		let view_id = editor.open_file(path.clone()).await.expect("open file");

		// Simulate pending load with token=1.
		editor.state.pending_file_loads.insert(path.clone(), 1);

		// Simulate user editing the buffer: mark the file buffer as modified.
		editor.state.core.buffers.get_buffer_mut(view_id).unwrap().set_modified(true);

		// Apply the load (correct token, but buffer is modified).
		let loaded_rope = Rope::from_str("disk content");
		editor.apply_loaded_file(path.clone(), loaded_rope, false, 1);

		let buf = editor.state.core.buffers.get_buffer(view_id).unwrap();
		assert!(buf.modified(), "buffer should remain modified");
		let content = buf.with_doc(|doc| doc.content().to_string());
		assert_ne!(content, "disk content", "loaded content should not overwrite modified buffer");

		// Pending load should be cleared (token matched, even though content was rejected).
		assert!(
			!editor.state.pending_file_loads.contains_key(&path),
			"pending load should be cleared even on rejection"
		);
	}

	#[tokio::test]
	async fn concurrent_file_loads_apply_by_path_out_of_order() {
		let mut editor = Editor::new_scratch();
		let path_a = PathBuf::from("/tmp/test_a.txt");
		let path_b = PathBuf::from("/tmp/test_b.txt");

		// Create buffers for both paths.
		let view_a = editor.open_file(path_a.clone()).await.expect("open a");
		let view_b = editor.open_file(path_b.clone()).await.expect("open b");

		// Simulate two concurrent pending loads with different tokens.
		editor.state.pending_file_loads.insert(path_a.clone(), 10);
		editor.state.pending_file_loads.insert(path_b.clone(), 20);

		// Apply B first (out of order).
		let rope_b = Rope::from_str("content B");
		editor.apply_loaded_file(path_b.clone(), rope_b, false, 20);

		// B should be populated, A should still be pending.
		let buf_b = editor.state.core.buffers.get_buffer(view_b).unwrap();
		assert_eq!(buf_b.with_doc(|doc| doc.content().to_string()), "content B");
		assert!(!editor.state.pending_file_loads.contains_key(&path_b), "B pending should be cleared");
		assert!(editor.state.pending_file_loads.contains_key(&path_a), "A pending should remain");

		// Now apply A.
		let rope_a = Rope::from_str("content A");
		editor.apply_loaded_file(path_a.clone(), rope_a, false, 10);

		let buf_a = editor.state.core.buffers.get_buffer(view_a).unwrap();
		assert_eq!(buf_a.with_doc(|doc| doc.content().to_string()), "content A");
		assert!(!editor.state.pending_file_loads.contains_key(&path_a), "A pending should be cleared");
	}
}
