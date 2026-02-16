//! Buffer creation and management operations.
//!
//! Opening files, creating buffers, and cloning for splits.

use std::path::PathBuf;

use xeno_registry::HookEventData;
use xeno_registry::hooks::{HookContext, emit as emit_hook, emit_sync_with as emit_hook_sync_with};

use super::{Editor, is_writable};
use crate::buffer::{Buffer, DocumentId, ViewId};
use crate::paste::normalize_to_lf;

impl Editor {
	/// Opens a new buffer from content, optionally with a path.
	///
	/// This async version awaits all hooks including async ones (e.g., LSP).
	/// For sync contexts like split operations, use [`open_buffer_sync`](Self::open_buffer_sync).
	pub async fn open_buffer(&mut self, content: String, path: Option<PathBuf>) -> ViewId {
		let buffer_id = self
			.state
			.core
			.buffers
			.create_buffer(content, path.clone(), &self.state.config.language_loader, self.state.viewport.width);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = self.state.core.buffers.get_buffer_mut(buffer_id).unwrap();

		let text_slice = buffer.with_doc(|doc| doc.content().clone());
		let file_type = buffer.file_type();
		emit_hook(&HookContext::new(HookEventData::BufferOpen {
			path: hook_path,
			text: text_slice.slice(..),
			file_type: file_type.as_deref(),
		}))
		.await;

		#[cfg(feature = "lsp")]
		self.maybe_track_lsp_for_buffer(buffer_id, false);

		buffer_id
	}

	/// Opens a new shared statehronously, scheduling async hooks for later.
	///
	/// Use this in sync contexts like split operations. Async hooks are queued
	/// in the hook runtime and will execute when the main loop drains them.
	pub fn open_buffer_sync(&mut self, content: String, path: Option<PathBuf>) -> ViewId {
		let buffer_id = self
			.state
			.core
			.buffers
			.create_buffer(content, path.clone(), &self.state.config.language_loader, self.state.viewport.width);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = self.state.core.buffers.get_buffer_mut(buffer_id).unwrap();

		let text = buffer.with_doc(|doc| doc.content().clone());
		emit_hook_sync_with(
			&HookContext::new(HookEventData::BufferOpen {
				path: hook_path,
				text: text.slice(..),
				file_type: buffer.file_type().as_deref(),
			}),
			&mut self.state.work_scheduler,
		);

		buffer_id
	}

	/// Opens a file as a new buffer.
	///
	/// Returns the new buffer's ID, or an error if the file couldn't be read.
	/// If the file exists but is not writable, the buffer is opened in readonly mode.
	pub async fn open_file(&mut self, path: PathBuf) -> anyhow::Result<ViewId> {
		let content = match tokio::fs::read_to_string(&path).await {
			Ok(s) => normalize_to_lf(s),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
			Err(e) => return Err(e.into()),
		};

		let readonly = path.exists() && !is_writable(&path);
		let buffer_id = self.open_buffer(content, Some(path)).await;

		if readonly && let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) {
			buffer.set_readonly(true);
		}

		Ok(buffer_id)
	}

	/// Builds a file-backed buffer for an existing view ID.
	pub(crate) async fn load_file_buffer_for_view(&mut self, view: ViewId, path: PathBuf) -> anyhow::Result<Buffer> {
		let content = match tokio::fs::read_to_string(&path).await {
			Ok(s) => normalize_to_lf(s),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
			Err(e) => return Err(e.into()),
		};

		let readonly = path.exists() && !is_writable(&path);
		let mut buffer = Buffer::new(view, content, Some(path));
		buffer.init_syntax(&self.state.config.language_loader);
		if let Some(width) = self.state.viewport.width {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}
		if readonly {
			buffer.set_readonly(true);
		}

		Ok(buffer)
	}

	/// Creates a new buffer that shares the same document as the current buffer.
	///
	/// This is used for split operations - both buffers see the same content
	/// but have independent cursor/selection/scroll state.
	pub fn clone_buffer_for_split(&mut self) -> ViewId {
		let focused = self.focused_view();
		self.state.core.buffers.clone_buffer_for_split(focused).expect("focused buffer must exist")
	}

	/// Initializes LSP for all currently open buffers.
	///
	/// Called after LSP servers are configured to handle buffers opened before
	/// server registration. Deduplicates by [`DocumentId`] to avoid redundant
	/// open notifications.
	#[cfg(feature = "lsp")]
	pub async fn init_lsp_for_open_buffers(&mut self) -> anyhow::Result<()> {
		let mut seen_docs = std::collections::HashSet::new();
		for buffer_id in self.state.core.buffers.buffer_ids().collect::<Vec<_>>() {
			let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
				continue;
			};
			if buffer.path().is_none() {
				continue;
			}
			if !seen_docs.insert(buffer.document_id()) {
				continue;
			}
			self.maybe_track_lsp_for_buffer(buffer_id, false);
		}
		Ok(())
	}

	/// Stub for non-LSP builds.
	#[cfg(not(feature = "lsp"))]
	pub async fn init_lsp_for_open_buffers(&mut self) -> anyhow::Result<()> {
		Ok(())
	}

	/// Spawns background LSP initialization for open buffers.
	///
	/// Called after first frame setup to ensure Time-To-First-Paint (TTFP) is
	/// not blocked by LSP server spawning. Deduplicates by [`DocumentId`].
	///
	/// Skips documents currently being loaded in the background to avoid
	/// initializing LSP with empty content.
	#[cfg(feature = "lsp")]
	pub fn kick_lsp_init_for_open_buffers(&mut self) {
		use std::collections::HashSet;

		let loading = self.state.loading_file.as_ref().map(|path| crate::paths::fast_abs(path));

		let mut seen_docs = HashSet::new();
		for buffer_id in self.state.core.buffers.buffer_ids().collect::<Vec<_>>() {
			let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
				continue;
			};
			if !seen_docs.insert(buffer.document_id()) {
				continue;
			}
			let Some(path) = buffer.path() else {
				continue;
			};
			let abs_path = crate::paths::fast_abs(&path);
			if loading.as_ref().is_some_and(|p| p == &abs_path) {
				continue;
			}
			self.maybe_track_lsp_for_buffer(buffer_id, false);
		}
	}

	#[cfg(not(feature = "lsp"))]
	pub fn kick_lsp_init_for_open_buffers(&mut self) {}

	#[cfg(feature = "lsp")]
	pub(crate) fn maybe_track_lsp_for_buffer(&mut self, buffer_id: ViewId, reset: bool) {
		if !self.state.lsp_catalog_ready {
			return;
		}

		let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		let Some(path) = buffer.path() else {
			return;
		};
		let Some(language) = buffer.file_type() else {
			return;
		};
		if self.state.lsp.registry().get_config(&language).is_none() {
			return;
		}

		let doc_id = buffer.document_id();
		let version = buffer.with_doc(|doc| doc.version());
		let supports_incremental = self.state.lsp.incremental_encoding_for_buffer(buffer).is_some();
		let config = crate::lsp::sync_manager::LspDocumentConfig {
			path: crate::paths::fast_abs(&path),
			language,
			supports_incremental,
		};

		if reset {
			self.state.lsp.sync_manager_mut().reset_tracked(doc_id, config, version);
		} else {
			self.state.lsp.sync_manager_mut().ensure_tracked(doc_id, config, version);
		}
	}

	/// Removes a buffer and performs final cleanup for its associated document.
	///
	/// If the removed buffer was the last view for its document, this method:
	/// 1. Invalidates the document in the [`RenderCache`].
	/// 2. Notifies the LSP sync manager to close the document.
	///
	/// This is the authoritative path for buffer destruction.
	///
	/// [`RenderCache`]: crate::render::cache::RenderCache
	pub(crate) fn finalize_buffer_removal(&mut self, id: ViewId) {
		let removed = self.state.core.buffers.remove_buffer_raw(id);
		if let Some(buffer) = removed {
			self.finalize_document_if_orphaned(buffer.document_id());
		}
	}

	/// Runs document-level cleanup when no views remain for a document.
	pub(crate) fn finalize_document_if_orphaned(&mut self, doc_id: DocumentId) {
		if self.state.core.buffers.any_buffer_for_doc(doc_id).is_some() {
			return;
		}

		#[cfg(feature = "lsp")]
		self.state.lsp.sync_manager_mut().on_doc_close(doc_id);

		self.state.syntax_manager.on_document_close(doc_id);
		self.state.render_cache.invalidate_document(doc_id);
	}
}
