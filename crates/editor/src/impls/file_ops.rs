//! File operations (save, load).
//!
//! Implements [`FileOpsAccess`] for the [`Editor`].

use std::io;
use std::path::{Path, PathBuf};

use ropey::Rope;
#[cfg(feature = "lsp")]
use tracing::warn;
use xeno_registry::commands::CommandError;
use xeno_registry::{HookContext, HookEventData, emit as emit_hook};

use super::Editor;

impl xeno_registry::FileOpsAccess for Editor {
	fn is_modified(&self) -> bool {
		self.buffer().modified()
	}

	fn save(
		&mut self,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), CommandError>> + '_>> {
		Box::pin(async move {
			let path_owned = match &self.buffer().path() {
				Some(p) => p.clone(),
				None => {
					return Err(CommandError::InvalidArgument(
						"No filename. Use :write <filename>".to_string(),
					));
				}
			};

			let text_slice = self.buffer().with_doc(|doc| doc.content().clone());
			emit_hook(&HookContext::new(
				HookEventData::BufferWritePre {
					path: &path_owned,
					text: text_slice.slice(..),
				},
				Some(&self.state.extensions),
			))
			.await;

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.lsp.on_buffer_will_save(self.buffer()) {
				warn!(error = %e, "LSP will_save notification failed");
			}

			let content = self.buffer().with_doc(|doc| {
				let mut content = Vec::new();
				for chunk in doc.content().chunks() {
					content.extend_from_slice(chunk.as_bytes());
				}
				content
			});

			if let Some(parent) = path_owned.parent()
				&& !parent.as_os_str().is_empty()
			{
				tokio::fs::create_dir_all(parent)
					.await
					.map_err(|e| CommandError::Io(e.to_string()))?;
			}

			tokio::fs::write(&path_owned, &content)
				.await
				.map_err(|e| CommandError::Io(e.to_string()))?;

			self.buffer_mut().set_modified(false);
			self.show_notification(xeno_registry::notifications::keys::file_saved(&path_owned));

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.lsp.on_buffer_did_save(self.buffer(), true) {
				warn!(error = %e, "LSP did_save notification failed");
			}

			emit_hook(&HookContext::new(
				HookEventData::BufferWrite { path: &path_owned },
				Some(&self.state.extensions),
			))
			.await;

			Ok(())
		})
	}

	fn save_as(
		&mut self,
		path: PathBuf,
	) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), CommandError>> + '_>> {
		self.buffer_mut().set_path(Some(path));
		self.save()
	}
}

impl Editor {
	/// Applies a loaded file to the editor.
	///
	/// Called by [`IoMsg::FileLoaded`] when background file loading completes.
	/// Replaces the buffer's content with the loaded rope and emits hooks.
	pub(crate) fn apply_loaded_file(&mut self, path: PathBuf, rope: Rope, readonly: bool) {
		tracing::debug!(path = %path.display(), len = rope.len_bytes(), "File loaded");

		self.state.loading_file = None;

		let Some(buffer_id) = self.state.core.buffers.find_by_path(&path) else {
			tracing::warn!(path = %path.display(), "No buffer found for loaded file");
			return;
		};

		let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) else {
			return;
		};

		buffer.reset_content(rope.clone());
		if readonly {
			buffer.set_readonly(true);
		}
		buffer.set_modified(false);
		buffer.set_cursor_and_selection(0, xeno_primitives::Selection::point(0));

		let file_type = buffer.file_type();
		crate::impls::emit_hook_sync_with(
			&HookContext::new(
				HookEventData::BufferOpen {
					path: &path,
					text: rope.slice(..),
					file_type: file_type.as_deref(),
				},
				None,
			),
			&mut self.state.hook_runtime,
		);

		if let Some((line, column)) = self.state.deferred_goto.take() {
			self.goto_line_col(line, column);
		}
	}

	/// Notifies the user of a file load error and clears loading state.
	pub(crate) fn notify_load_error(&mut self, path: &Path, error: &io::Error) {
		if self.state.loading_file.as_deref() == Some(path) {
			self.state.loading_file = None;
		}
		self.show_notification(xeno_registry::notifications::keys::error(format!(
			"Failed to load {}: {}",
			path.display(),
			error
		)));
	}

	/// Returns the path of the file currently being loaded, if any.
	pub fn loading_file(&self) -> Option<&Path> {
		self.state.loading_file.as_deref()
	}
}
