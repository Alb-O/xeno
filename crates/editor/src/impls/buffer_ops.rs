//! Buffer creation and management operations.
//!
//! Opening files, creating buffers, and cloning for splits.

use std::path::PathBuf;

#[cfg(feature = "lsp")]
use tracing::warn;
use xeno_registry::{
	HookContext, HookEventData, emit as emit_hook, emit_sync_with as emit_hook_sync_with,
};

use super::{Editor, is_writable};
use crate::buffer::BufferId;

impl Editor {
	/// Opens a new buffer from content, optionally with a path.
	///
	/// This async version awaits all hooks including async ones (e.g., LSP).
	/// For sync contexts like split operations, use [`open_buffer_sync`](Self::open_buffer_sync).
	pub async fn open_buffer(&mut self, content: String, path: Option<PathBuf>) -> BufferId {
		let buffer_id = self.core.buffers.create_buffer(
			content,
			path.clone(),
			&self.config.language_loader,
			self.viewport.width,
		);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = self.core.buffers.get_buffer(buffer_id).unwrap();

		let text_slice = buffer.with_doc(|doc| doc.content().clone());
		let file_type = buffer.file_type();
		emit_hook(&HookContext::new(
			HookEventData::BufferOpen {
				path: hook_path,
				text: text_slice.slice(..),
				file_type: file_type.as_deref(),
			},
			Some(&self.extensions),
		))
		.await;

		#[cfg(feature = "lsp")]
		if let Some(buffer) = self.core.buffers.get_buffer(buffer_id)
			&& let Err(e) = self.lsp.on_buffer_open(buffer).await
		{
			warn!(error = %e, "LSP buffer open failed");
		}

		buffer_id
	}

	/// Opens a new buffer synchronously, scheduling async hooks for later.
	///
	/// Use this in sync contexts like split operations. Async hooks are queued
	/// in the hook runtime and will execute when the main loop drains them.
	pub fn open_buffer_sync(&mut self, content: String, path: Option<PathBuf>) -> BufferId {
		let buffer_id = self.core.buffers.create_buffer(
			content,
			path.clone(),
			&self.config.language_loader,
			self.viewport.width,
		);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = self.core.buffers.get_buffer(buffer_id).unwrap();

		let text = buffer.with_doc(|doc| doc.content().clone());
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::BufferOpen {
					path: hook_path,
					text: text.slice(..),
					file_type: buffer.file_type().as_deref(),
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);

		buffer_id
	}

	/// Opens a file as a new buffer.
	///
	/// Returns the new buffer's ID, or an error if the file couldn't be read.
	/// If the file exists but is not writable, the buffer is opened in readonly mode.
	pub async fn open_file(&mut self, path: PathBuf) -> anyhow::Result<BufferId> {
		let content = match tokio::fs::read_to_string(&path).await {
			Ok(s) => s,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
			Err(e) => return Err(e.into()),
		};

		let readonly = path.exists() && !is_writable(&path);
		let buffer_id = self.open_buffer(content, Some(path)).await;

		if readonly && let Some(buffer) = self.core.buffers.get_buffer(buffer_id) {
			buffer.set_readonly(true);
		}

		Ok(buffer_id)
	}

	/// Creates a new buffer that shares the same document as the current buffer.
	///
	/// This is used for split operations - both buffers see the same content
	/// but have independent cursor/selection/scroll state.
	pub fn clone_buffer_for_split(&mut self) -> BufferId {
		self.core.buffers.clone_focused_buffer_for_split()
	}

	/// Initializes LSP for all currently open buffers.
	///
	/// This is called after LSP servers are configured to handle buffers
	/// that were opened before the servers were registered.
	#[cfg(feature = "lsp")]
	pub async fn init_lsp_for_open_buffers(&mut self) -> anyhow::Result<()> {
		for buffer_id in self.core.buffers.buffer_ids().collect::<Vec<_>>() {
			if let Some(buffer) = self.core.buffers.get_buffer(buffer_id)
				&& buffer.path().is_some()
				&& let Err(e) = self.lsp.on_buffer_open(buffer).await
			{
				warn!(error = %e, "Failed to initialize LSP for buffer");
			}
		}
		Ok(())
	}

	/// Stub for non-LSP builds.
	#[cfg(not(feature = "lsp"))]
	pub async fn init_lsp_for_open_buffers(&mut self) -> anyhow::Result<()> {
		Ok(())
	}
}
