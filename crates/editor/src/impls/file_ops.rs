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
					return Err(CommandError::InvalidArgument(
						"No filename. Use :write <filename>".to_string(),
					));
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
				tokio::fs::create_dir_all(parent)
					.await
					.map_err(|e| CommandError::Io(e.to_string()))?;
			}

			tokio::fs::write(&path_owned, &content)
				.await
				.map_err(|e| CommandError::Io(e.to_string()))?;

			let _ = self.buffer_mut().set_modified(false);
			self.show_notification(xeno_registry::notifications::keys::file_saved(&path_owned));

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.lsp.on_buffer_did_save(self.buffer(), true).await {
				warn!(error = %e, "LSP did_save notification failed");
			}

			emit_hook(&HookContext::new(HookEventData::BufferWrite {
				path: &path_owned,
			}))
			.await;

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
		self.state.syntax_manager.reset_syntax(buffer.document_id());
		if readonly {
			buffer.set_readonly(true);
		}
		buffer.set_modified(false);
		buffer.set_cursor_and_selection(0, xeno_primitives::Selection::point(0));

		let file_type = buffer.file_type();

		#[cfg(feature = "lsp")]
		{
			let doc_id = buffer.document_id();

			// Initialize standard LSP session
			if let Some(language) = &file_type
				&& self.state.lsp_catalog_ready
				&& self.state.lsp.registry().get_config(language).is_some()
			{
				let lsp_path = if path.is_absolute() {
					path.clone()
				} else {
					std::env::current_dir().unwrap_or_default().join(&path)
				};

				let version = buffer.with_doc(|doc| doc.version());
				let supports_incremental = self
					.state
					.lsp
					.incremental_encoding_for_buffer(buffer)
					.is_some();

				self.state.lsp.sync_manager_mut().on_doc_open(
					doc_id,
					crate::lsp::sync_manager::LspDocumentConfig {
						path: lsp_path.clone(),
						language: language.clone(),
						supports_incremental,
					},
					version,
				);

				let sync = self.state.lsp.sync_clone();
				let path_for_lsp = lsp_path;
				let rope_for_lsp = rope.clone();
				let language = language.clone();
				tokio::spawn(async move {
					if let Some(uri) = xeno_lsp::uri_from_path(&path_for_lsp)
						&& sync.documents().is_opened(&uri)
					{
						return;
					}

					let content =
						match tokio::task::spawn_blocking(move || rope_for_lsp.to_string()).await {
							Ok(content) => content,
							Err(e) => {
								tracing::warn!(
									path = %path_for_lsp.display(),
									language = %language,
									error = %e,
									"LSP snapshot conversion failed"
								);
								return;
							}
						};

					if let Err(e) = sync
						.open_document_text(&path_for_lsp, &language, content)
						.await
					{
						tracing::warn!(path = %path_for_lsp.display(), language, error = %e, "Async LSP init failed");
					}
				});
			}
		}

		crate::impls::emit_hook_sync_with(
			&HookContext::new(HookEventData::BufferOpen {
				path: &path,
				text: rope.slice(..),
				file_type: file_type.as_deref(),
			}),
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
