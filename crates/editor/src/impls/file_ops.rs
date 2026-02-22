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
	///
	/// Delegates the atomic write to [`crate::io::save_buffer_to_disk`],
	/// wrapping it with hooks, LSP notifications, and post-save state
	/// updates (modified flag, user notification).
	pub fn save(&mut self) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			let path_owned = match &self.buffer().path() {
				Some(p) => p.clone(),
				None => {
					return Err(CommandError::InvalidArgument("No filename. Use :write <filename>".to_string()));
				}
			};

			// Snapshot content for hooks before save.
			let rope = self.buffer().with_doc(|doc| doc.content().clone());

			emit_hook(&HookContext::new(HookEventData::BufferWritePre {
				path: &path_owned,
				text: rope.slice(..),
			}))
			.await;

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.integration.lsp.on_buffer_will_save(self.buffer()).await {
				warn!(error = %e, "LSP will_save notification failed");
			}

			if let Some(parent) = path_owned.parent()
				&& !parent.as_os_str().is_empty()
			{
				tokio::fs::create_dir_all(parent).await.map_err(|e| CommandError::Io(e.to_string()))?;
			}

			let buffer_id = self.focused_view();
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer(buffer_id)
				.ok_or_else(|| CommandError::Io("buffer not found".to_string()))?;
			crate::io::save_buffer_to_disk(buffer).await.map_err(|e| CommandError::Io(e.to_string()))?;

			let _ = self.buffer_mut().set_modified(false);
			self.show_notification(xeno_registry::notifications::keys::file_saved(&path_owned));

			#[cfg(feature = "lsp")]
			if let Err(e) = self.state.integration.lsp.on_buffer_did_save(self.buffer(), true).await {
				warn!(error = %e, "LSP did_save notification failed");
			}

			emit_hook(&HookContext::new(HookEventData::BufferWrite { path: &path_owned })).await;

			Ok(())
		})
	}

	/// Saves the current buffer to a new file path.
	///
	/// This is a "copy+switch" operation: the old file remains on disk and the
	/// buffer path is updated to the new location. This does NOT send
	/// `willRenameFiles`/`didRenameFiles` to LSP servers since no file is being
	/// moved or deleted.
	pub fn save_as(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		let loader_arc = self.state.config.config.language_loader.clone();
		let _ = self.buffer_mut().set_path(Some(path), Some(&loader_arc));
		#[cfg(feature = "lsp")]
		{
			let buf_id = self.focused_view();
			self.maybe_track_lsp_for_buffer(buf_id, true);
		}
		self.save()
	}

	/// Renames/moves the current buffer's file on disk.
	///
	/// Sends `workspace/willRenameFiles` before the rename to get import-path
	/// updates from the language server, performs `std::fs::rename`, updates
	/// the buffer path and LSP identity, then sends `workspace/didRenameFiles`.
	pub fn rename_current_file(&mut self, new_path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			let old_path = self
				.buffer()
				.path()
				.map(|p| p.to_path_buf())
				.ok_or_else(|| CommandError::InvalidArgument("Buffer has no file path".into()))?;

			if self.buffer().modified() {
				return Err(CommandError::Failed("Buffer has unsaved changes — save first".into()));
			}

			// Resolve new_path relative to old_path's parent directory.
			let new_path = if new_path.is_relative() {
				old_path.parent().unwrap_or_else(|| std::path::Path::new(".")).join(&new_path)
			} else {
				new_path
			};

			if new_path == old_path {
				return Ok(());
			}

			// Canonicalize paths BEFORE the filesystem rename so the old path
			// still exists on disk and resolves symlinks correctly.
			#[cfg(feature = "lsp")]
			let abs_old = self.state.integration.lsp.canonicalize_path(&old_path);
			#[cfg(feature = "lsp")]
			let abs_new = self.state.integration.lsp.canonicalize_path(&new_path);
			#[cfg(feature = "lsp")]
			let old_language = self.buffer().file_type().map(|s| s.to_string());

			// Build consistent FileRename URIs used for will/did/reopen.
			#[cfg(feature = "lsp")]
			let file_rename = {
				let old_uri = xeno_lsp::uri_from_path(&abs_old).map(|u| u.to_string());
				let new_uri = xeno_lsp::uri_from_path(&abs_new).map(|u| u.to_string());
				old_uri
					.zip(new_uri)
					.map(|(old_uri, new_uri)| xeno_lsp::lsp_types::FileRename { old_uri, new_uri })
			};

			// Ask the language server for import-path edits before renaming.
			#[cfg(feature = "lsp")]
			let lsp_client = {
				let client = old_language
					.as_deref()
					.and_then(|lang| self.state.integration.lsp.sync().registry().get(lang, &abs_old).filter(|c| c.is_ready()));

				if let (Some(client), Some(rename)) = (&client, &file_rename) {
					match client.will_rename_files(vec![rename.clone()]).await {
						Ok(Some(edit)) => {
							let text_only = Self::filter_text_only_edit(edit);
							if text_only.changes.as_ref().is_some_and(|c| !c.is_empty()) || text_only.document_changes.is_some() {
								if let Err(e) = self.apply_workspace_edit(text_only).await {
									warn!(error = %e.error, "willRenameFiles workspace edit failed");
								}
							}
						}
						Err(e) => {
							warn!(error = %e, "willRenameFiles request failed");
						}
						_ => {}
					}
				}
				client
			};

			// Create parent directories if needed.
			if let Some(parent) = new_path.parent()
				&& !parent.as_os_str().is_empty()
			{
				tokio::fs::create_dir_all(parent).await.map_err(|e| CommandError::Io(e.to_string()))?;
			}

			// Perform the actual filesystem rename.
			match tokio::fs::rename(&old_path, &new_path).await {
				Ok(()) => {}
				Err(e) if Self::is_cross_device_rename(&e) => {
					return Err(CommandError::Failed(format!(
						"Cross-device rename not supported (EXDEV): {} -> {}",
						old_path.display(),
						new_path.display()
					)));
				}
				Err(e) => return Err(CommandError::Io(e.to_string())),
			}

			// Update buffer path.
			let loader_arc = self.state.config.config.language_loader.clone();
			let _ = self.buffer_mut().set_path(Some(new_path.clone()), Some(&loader_arc));

			// Reopen the LSP document: didClose(old URI) + didOpen(new URI).
			// Uses pre-rename canonicalized paths for URI consistency.
			#[cfg(feature = "lsp")]
			{
				let new_language = self.buffer().file_type().map(|s| s.to_string());
				let old_lang = old_language.as_deref().or(new_language.as_deref());
				let new_lang = new_language.as_deref().or(old_language.as_deref());
				if let (Some(old_l), Some(new_l)) = (old_lang, new_lang) {
					let text = self.buffer().with_doc(|doc| doc.content().to_string());
					if let Err(e) = self.state.integration.lsp.sync().reopen_document(&abs_old, old_l, &abs_new, new_l, text).await {
						warn!(error = %e, "LSP reopen_document after rename failed");
					}
				}
				let buf_id = self.focused_view();
				self.maybe_track_lsp_for_buffer(buf_id, true);
			}

			self.show_notification(xeno_registry::notifications::keys::info(format!("Renamed to {}", new_path.display())));

			// Notify the server that the file was renamed (reuse same client + URIs).
			#[cfg(feature = "lsp")]
			if let (Some(client), Some(rename)) = (lsp_client, file_rename) {
				if let Err(e) = client.did_rename_files(vec![rename]).await {
					warn!(error = %e, "didRenameFiles notification failed");
				}
			}

			Ok(())
		})
	}

	/// Creates a new file on disk and opens it in the editor.
	///
	/// Sends `workspace/willCreateFiles` before creation and
	/// `workspace/didCreateFiles` after. If the file already exists, opens it
	/// without sending LSP file operation hooks.
	pub fn create_file(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			// Resolve relative to current buffer's parent or cwd.
			let path = if path.is_relative() {
				let base = self
					.buffer()
					.path()
					.and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
					.unwrap_or_else(|| std::path::PathBuf::from("."));
				base.join(&path)
			} else {
				path
			};

			// If file already exists, just open it (no LSP create hooks).
			if path.exists() {
				let _ = self.open_file(path).await.map_err(|e| CommandError::Failed(e.to_string()))?;
				return Ok(());
			}

			// Derive language from target path for LSP client lookup.
			#[cfg(feature = "lsp")]
			let abs_path = self.state.integration.lsp.canonicalize_path(&path);
			#[cfg(feature = "lsp")]
			let target_language = self
				.state
				.config
				.config
				.language_loader
				.language_for_path(&path)
				.and_then(|id| self.state.config.config.language_loader.get(id))
				.map(|l| l.name().to_string());
			#[cfg(feature = "lsp")]
			let file_create = xeno_lsp::uri_from_path(&abs_path).map(|u| xeno_lsp::lsp_types::FileCreate { uri: u.to_string() });

			// Ask server for edits before file creation.
			#[cfg(feature = "lsp")]
			let lsp_client = {
				let client = target_language
					.as_deref()
					.and_then(|lang| self.state.integration.lsp.sync().registry().get(lang, &abs_path).filter(|c| c.is_ready()));
				if let (Some(client), Some(fc)) = (&client, &file_create) {
					match client.will_create_files(vec![fc.clone()]).await {
						Ok(Some(edit)) => {
							let text_only = Self::filter_text_only_edit(edit);
							if text_only.changes.as_ref().is_some_and(|c| !c.is_empty()) || text_only.document_changes.is_some() {
								if let Err(e) = self.apply_workspace_edit(text_only).await {
									warn!(error = %e.error, "willCreateFiles workspace edit failed");
								}
							}
						}
						Err(e) => warn!(error = %e, "willCreateFiles request failed"),
						_ => {}
					}
				}
				client
			};

			// Create parent directories + file.
			if let Some(parent) = path.parent()
				&& !parent.as_os_str().is_empty()
			{
				tokio::fs::create_dir_all(parent).await.map_err(|e| CommandError::Io(e.to_string()))?;
			}
			// Use create_new to avoid TOCTOU clobber if file appeared between exists() check and here.
			match tokio::fs::OpenOptions::new().write(true).create_new(true).open(&path).await {
				Ok(_) => {}
				Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
					// Race: file appeared; just open it (skip didCreateFiles).
					let _ = self.open_file(path).await.map_err(|e| CommandError::Failed(e.to_string()))?;
					return Ok(());
				}
				Err(e) => return Err(CommandError::Io(e.to_string())),
			}

			// Open the file in the editor (triggers didOpen via LSP tracking).
			let _ = self.open_file(path).await.map_err(|e| CommandError::Failed(e.to_string()))?;

			// Notify server after didOpen so sequence is: willCreate → didOpen → didCreate.
			#[cfg(feature = "lsp")]
			if let (Some(client), Some(fc)) = (lsp_client, file_create) {
				if let Err(e) = client.did_create_files(vec![fc]).await {
					warn!(error = %e, "didCreateFiles notification failed");
				}
			}

			Ok(())
		})
	}

	/// Deletes the current buffer's file from disk.
	///
	/// Sends `workspace/willDeleteFiles` before deletion and
	/// `workspace/didDeleteFiles` after. The buffer's LSP document is
	/// explicitly closed before the didDelete notification to ensure
	/// `didClose` precedes `didDeleteFiles`.
	pub fn delete_current_file(&mut self) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			let path = self
				.buffer()
				.path()
				.map(|p| p.to_path_buf())
				.ok_or_else(|| CommandError::InvalidArgument("Buffer has no file path".into()))?;

			if self.buffer().modified() {
				return Err(CommandError::Failed("Buffer has unsaved changes".into()));
			}

			// Canonicalize BEFORE deletion (while file still exists).
			#[cfg(feature = "lsp")]
			let abs_path = self.state.integration.lsp.canonicalize_path(&path);
			#[cfg(feature = "lsp")]
			let language = self.buffer().file_type().map(|s| s.to_string());
			#[cfg(feature = "lsp")]
			let file_delete = xeno_lsp::uri_from_path(&abs_path).map(|u| xeno_lsp::lsp_types::FileDelete { uri: u.to_string() });

			// Ask server for text-only edits before deletion.
			#[cfg(feature = "lsp")]
			let lsp_client = {
				let client = language
					.as_deref()
					.and_then(|lang| self.state.integration.lsp.sync().registry().get(lang, &abs_path).filter(|c| c.is_ready()));
				if let (Some(client), Some(fd)) = (&client, &file_delete) {
					match client.will_delete_files(vec![fd.clone()]).await {
						Ok(Some(edit)) => {
							let text_only = Self::filter_text_only_edit(edit);
							if text_only.changes.as_ref().is_some_and(|c| !c.is_empty()) || text_only.document_changes.is_some() {
								if let Err(e) = self.apply_workspace_edit(text_only).await {
									warn!(error = %e.error, "willDeleteFiles workspace edit failed");
								}
							}
						}
						Err(e) => warn!(error = %e, "willDeleteFiles request failed"),
						_ => {}
					}
				}
				client
			};

			// Delete the file from disk.
			tokio::fs::remove_file(&path).await.map_err(|e| CommandError::Io(e.to_string()))?;

			// Close LSP document explicitly (didClose) BEFORE didDeleteFiles.
			#[cfg(feature = "lsp")]
			if let Some(lang) = language.as_deref() {
				if let Err(e) = self.state.integration.lsp.sync().close_document(&abs_path, lang).await {
					warn!(error = %e, "LSP close_document after delete failed");
				}
			}

			// Notify server that files were deleted.
			#[cfg(feature = "lsp")]
			if let (Some(client), Some(fd)) = (lsp_client, file_delete) {
				if let Err(e) = client.did_delete_files(vec![fd]).await {
					warn!(error = %e, "didDeleteFiles notification failed");
				}
			}

			self.show_notification(xeno_registry::notifications::keys::info(format!("Deleted {}", path.display())));

			// Close the buffer in editor state.
			let buf_id = self.focused_view();
			self.close_buffer(buf_id);

			Ok(())
		})
	}

	/// Filters a `WorkspaceEdit` to only include text edits, dropping
	/// resource operations (create/rename/delete) to prevent double effects
	/// when the editor manages the resource operation itself.
	#[cfg(feature = "lsp")]
	fn filter_text_only_edit(edit: xeno_lsp::lsp_types::WorkspaceEdit) -> xeno_lsp::lsp_types::WorkspaceEdit {
		let document_changes = edit.document_changes.and_then(|dcs| match dcs {
			xeno_lsp::lsp_types::DocumentChanges::Edits(edits) => Some(xeno_lsp::lsp_types::DocumentChanges::Edits(edits)),
			xeno_lsp::lsp_types::DocumentChanges::Operations(ops) => {
				let text_ops: Vec<_> = ops
					.into_iter()
					.filter(|op| matches!(op, xeno_lsp::lsp_types::DocumentChangeOperation::Edit(_)))
					.collect();
				if text_ops.is_empty() {
					None
				} else {
					Some(xeno_lsp::lsp_types::DocumentChanges::Operations(text_ops))
				}
			}
		});
		xeno_lsp::lsp_types::WorkspaceEdit {
			changes: edit.changes,
			document_changes,
			change_annotations: edit.change_annotations,
		}
	}

	/// Detects cross-device rename errors (EXDEV = 18 on Linux/macOS).
	fn is_cross_device_rename(e: &std::io::Error) -> bool {
		#[cfg(unix)]
		{
			e.raw_os_error() == Some(18) // EXDEV
		}
		#[cfg(not(unix))]
		{
			_ = e;
			false
		}
	}

	/// Creates a directory on disk with LSP `fileOperations` hooks.
	///
	/// Broadcasts `willCreateFiles`/`didCreateFiles` to all ready LSP clients
	/// since directories are language-agnostic. Creates intermediate
	/// directories as needed.
	pub fn create_dir(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			// Resolve relative to current buffer's parent or cwd.
			let path = if path.is_relative() {
				let base = self
					.buffer()
					.path()
					.and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
					.unwrap_or_else(|| std::path::PathBuf::from("."));
				base.join(&path)
			} else {
				path
			};

			// If path already exists, check it's a directory and return early (no LSP hooks).
			if path.exists() {
				if !path.is_dir() {
					return Err(CommandError::Failed(format!("Path exists and is not a directory: {}", path.display())));
				}
				self.show_notification(xeno_registry::notifications::keys::info(format!(
					"Directory already exists: {}",
					path.display()
				)));
				return Ok(());
			}

			#[cfg(feature = "lsp")]
			let abs_path = self.state.integration.lsp.canonicalize_path(&path);
			#[cfg(feature = "lsp")]
			let file_create = xeno_lsp::uri_from_path(&abs_path).map(|u| xeno_lsp::lsp_types::FileCreate { uri: u.to_string() });

			// Snapshot ready clients once for consistent will/did recipients.
			#[cfg(feature = "lsp")]
			let lsp_clients = self.state.integration.lsp.sync().registry().ready_clients();

			// Broadcast willCreateFiles to all ready clients.
			#[cfg(feature = "lsp")]
			if let Some(fc) = &file_create {
				for client in &lsp_clients {
					match client.will_create_files(vec![fc.clone()]).await {
						Ok(Some(edit)) => {
							let text_only = Self::filter_text_only_edit(edit);
							if text_only.changes.as_ref().is_some_and(|c| !c.is_empty()) || text_only.document_changes.is_some() {
								if let Err(e) = self.apply_workspace_edit(text_only).await {
									warn!(error = %e.error, "willCreateFiles workspace edit failed");
								}
							}
						}
						Err(e) => warn!(error = %e, "willCreateFiles request failed"),
						_ => {}
					}
				}
			}

			tokio::fs::create_dir_all(&path).await.map_err(|e| CommandError::Io(e.to_string()))?;

			// Broadcast didCreateFiles.
			#[cfg(feature = "lsp")]
			if let Some(fc) = file_create {
				for client in &lsp_clients {
					if let Err(e) = client.did_create_files(vec![fc.clone()]).await {
						warn!(error = %e, "didCreateFiles notification failed");
					}
				}
			}

			self.show_notification(xeno_registry::notifications::keys::info(format!("Created directory {}", path.display())));
			Ok(())
		})
	}

	/// Deletes an empty directory from disk with LSP `fileOperations` hooks.
	///
	/// Only removes empty directories. Returns an error if the directory
	/// is non-empty. Broadcasts `willDeleteFiles`/`didDeleteFiles` to all
	/// ready LSP clients.
	pub fn delete_dir(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			// Resolve relative to current buffer's parent or cwd.
			let path = if path.is_relative() {
				let base = self
					.buffer()
					.path()
					.and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
					.unwrap_or_else(|| std::path::PathBuf::from("."));
				base.join(&path)
			} else {
				path
			};

			if !path.is_dir() {
				return Err(CommandError::InvalidArgument(format!("Not a directory: {}", path.display())));
			}

			#[cfg(feature = "lsp")]
			let abs_path = self.state.integration.lsp.canonicalize_path(&path);
			#[cfg(feature = "lsp")]
			let file_delete = xeno_lsp::uri_from_path(&abs_path).map(|u| xeno_lsp::lsp_types::FileDelete { uri: u.to_string() });

			// Snapshot ready clients once for consistent will/did recipients.
			#[cfg(feature = "lsp")]
			let lsp_clients = self.state.integration.lsp.sync().registry().ready_clients();

			// Broadcast willDeleteFiles to all ready clients.
			#[cfg(feature = "lsp")]
			if let Some(fd) = &file_delete {
				for client in &lsp_clients {
					match client.will_delete_files(vec![fd.clone()]).await {
						Ok(Some(edit)) => {
							let text_only = Self::filter_text_only_edit(edit);
							if text_only.changes.as_ref().is_some_and(|c| !c.is_empty()) || text_only.document_changes.is_some() {
								if let Err(e) = self.apply_workspace_edit(text_only).await {
									warn!(error = %e.error, "willDeleteFiles workspace edit failed");
								}
							}
						}
						Err(e) => warn!(error = %e, "willDeleteFiles request failed"),
						_ => {}
					}
				}
			}

			// remove_dir (not remove_dir_all) — only empty dirs.
			tokio::fs::remove_dir(&path).await.map_err(|e| {
				// ENOTEMPTY = 39 on Linux, 66 on macOS
				if e.raw_os_error() == Some(39) || e.raw_os_error() == Some(66) {
					CommandError::Failed(format!("Directory not empty: {}", path.display()))
				} else {
					CommandError::Io(e.to_string())
				}
			})?;

			// Broadcast didDeleteFiles.
			#[cfg(feature = "lsp")]
			if let Some(fd) = file_delete {
				for client in &lsp_clients {
					if let Err(e) = client.did_delete_files(vec![fd.clone()]).await {
						warn!(error = %e, "didDeleteFiles notification failed");
					}
				}
			}

			self.show_notification(xeno_registry::notifications::keys::info(format!("Deleted directory {}", path.display())));
			Ok(())
		})
	}

	/// Applies a loaded file to the editor.
	///
	/// Called by [`crate::msg::IoMsg::FileLoaded`] when background file loading completes.
	/// Token-gated: ignores stale loads (superseded by a newer request). Also
	/// refuses to overwrite a buffer that has been modified since the load was
	/// kicked, preserving user edits.
	pub(crate) fn apply_loaded_file(&mut self, path: PathBuf, rope: Rope, readonly: bool, token: u64) {
		// Stale token check: only apply if this token matches the pending load for this path.
		let is_current = self.state.async_state.pending_file_loads.get(&path) == Some(&token);
		if !is_current {
			tracing::debug!(path = %path.display(), token, "Ignoring stale file load");
			return;
		}

		self.state.async_state.pending_file_loads.remove(&path);

		let Some(buffer_id) = self.state.core.editor.buffers.find_by_path(&path) else {
			tracing::warn!(path = %path.display(), "No buffer found for loaded file");
			return;
		};

		let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(buffer_id) else {
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
		self.state.integration.syntax_manager.reset_syntax(buffer.document_id());
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
			&mut self.state.integration.work_scheduler,
		);

		if let Some((line, column)) = self.state.async_state.deferred_goto.take() {
			self.goto_line_col(line, column);
		}
	}

	/// Notifies the user of a file load error and clears loading state.
	pub(crate) fn notify_load_error(&mut self, path: &Path, error: &io::Error, token: u64) {
		if self.state.async_state.pending_file_loads.get(path) == Some(&token) {
			self.state.async_state.pending_file_loads.remove(path);
		}
		self.show_notification(xeno_registry::notifications::keys::error(format!(
			"Failed to load {}: {}",
			path.display(),
			error
		)));
	}

	/// Returns true if the given path has a pending background load.
	pub fn is_loading_file(&self, path: &Path) -> bool {
		self.state.async_state.pending_file_loads.contains_key(path)
	}

	/// Returns true if any file is currently being loaded in the background.
	pub fn has_pending_file_loads(&self) -> bool {
		!self.state.async_state.pending_file_loads.is_empty()
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
		editor.state.async_state.pending_file_loads.insert(path.clone(), 2);

		// Apply a stale load (token=1) — should be ignored.
		let stale_rope = Rope::from_str("stale content");
		editor.apply_loaded_file(path.clone(), stale_rope, false, 1);

		let buf = editor.state.core.editor.buffers.get_buffer(view_id).unwrap();
		let content = buf.with_doc(|doc| doc.content().to_string());
		assert_ne!(content, "stale content", "stale token should not overwrite buffer");

		// Pending load should still be active (not cleared by the stale token).
		assert!(
			editor.state.async_state.pending_file_loads.contains_key(&path),
			"pending load should remain for the current token"
		);

		// Apply the current load (token=2) — should succeed.
		let current_rope = Rope::from_str("current content");
		editor.apply_loaded_file(path.clone(), current_rope, false, 2);

		let buf = editor.state.core.editor.buffers.get_buffer(view_id).unwrap();
		let content = buf.with_doc(|doc| doc.content().to_string());
		assert_eq!(content, "current content", "current token should replace buffer content");
		assert!(
			!editor.state.async_state.pending_file_loads.contains_key(&path),
			"pending load should be cleared"
		);
	}

	#[tokio::test]
	async fn file_loaded_does_not_clobber_modified_buffer() {
		let mut editor = Editor::new_scratch();
		let path = PathBuf::from("/tmp/test_modified.txt");

		// Create a buffer for the path and track its view ID.
		let view_id = editor.open_file(path.clone()).await.expect("open file");

		// Simulate pending load with token=1.
		editor.state.async_state.pending_file_loads.insert(path.clone(), 1);

		// Simulate user editing the buffer: mark the file buffer as modified.
		editor.state.core.editor.buffers.get_buffer_mut(view_id).unwrap().set_modified(true);

		// Apply the load (correct token, but buffer is modified).
		let loaded_rope = Rope::from_str("disk content");
		editor.apply_loaded_file(path.clone(), loaded_rope, false, 1);

		let buf = editor.state.core.editor.buffers.get_buffer(view_id).unwrap();
		assert!(buf.modified(), "buffer should remain modified");
		let content = buf.with_doc(|doc| doc.content().to_string());
		assert_ne!(content, "disk content", "loaded content should not overwrite modified buffer");

		// Pending load should be cleared (token matched, even though content was rejected).
		assert!(
			!editor.state.async_state.pending_file_loads.contains_key(&path),
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
		editor.state.async_state.pending_file_loads.insert(path_a.clone(), 10);
		editor.state.async_state.pending_file_loads.insert(path_b.clone(), 20);

		// Apply B first (out of order).
		let rope_b = Rope::from_str("content B");
		editor.apply_loaded_file(path_b.clone(), rope_b, false, 20);

		// B should be populated, A should still be pending.
		let buf_b = editor.state.core.editor.buffers.get_buffer(view_b).unwrap();
		assert_eq!(buf_b.with_doc(|doc| doc.content().to_string()), "content B");
		assert!(
			!editor.state.async_state.pending_file_loads.contains_key(&path_b),
			"B pending should be cleared"
		);
		assert!(editor.state.async_state.pending_file_loads.contains_key(&path_a), "A pending should remain");

		// Now apply A.
		let rope_a = Rope::from_str("content A");
		editor.apply_loaded_file(path_a.clone(), rope_a, false, 10);

		let buf_a = editor.state.core.editor.buffers.get_buffer(view_a).unwrap();
		assert_eq!(buf_a.with_doc(|doc| doc.content().to_string()), "content A");
		assert!(
			!editor.state.async_state.pending_file_loads.contains_key(&path_a),
			"A pending should be cleared"
		);
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn save_preserves_file_permissions() {
		use std::os::unix::fs::PermissionsExt;

		use xeno_primitives::{SyntaxPolicy, Transaction, UndoPolicy};

		use crate::buffer::ApplyPolicy;

		let dir = std::env::temp_dir().join("xeno_test_save_perms");
		std::fs::create_dir_all(&dir).unwrap();
		let path = dir.join("perms_test.rs");
		std::fs::write(&path, "original\n").unwrap();
		std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();

		let mut editor = Editor::new_scratch();
		let view_id = editor.open_file(path.clone()).await.unwrap();

		// Edit the buffer to make it modified.
		{
			let buffer = editor.state.core.editor.buffers.get_buffer_mut(view_id).unwrap();
			let tx = buffer.with_doc(|doc| {
				Transaction::change(
					doc.content().slice(..),
					vec![xeno_primitives::Change {
						start: 0,
						end: 8,
						replacement: Some("modified".into()),
					}],
				)
			});
			buffer.apply(
				&tx,
				ApplyPolicy {
					undo: UndoPolicy::Record,
					syntax: SyntaxPolicy::IncrementalOrDirty,
				},
			);
		}
		assert!(editor.state.core.editor.buffers.get_buffer(view_id).unwrap().modified());

		// Switch focus to the buffer and save.
		let base_window = editor.state.core.windows.base_id();
		editor.state.core.focus = crate::impls::focus::FocusTarget::Buffer {
			window: base_window,
			buffer: view_id,
		};
		editor.save().await.unwrap();

		// Disk content updated.
		assert_eq!(std::fs::read_to_string(&path).unwrap(), "modified\n");

		// Permissions preserved.
		let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
		assert_eq!(mode, 0o640, "save must preserve original file permissions");

		let _ = std::fs::remove_file(path);
		let _ = std::fs::remove_dir(dir);
	}

	#[tokio::test]
	async fn rename_current_file_moves_on_disk_and_updates_buffer() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let old_path = tmp.path().join("old.txt");
		std::fs::write(&old_path, "hello\n").expect("write");

		let mut editor = Editor::new(old_path.clone()).await.expect("open");
		assert!(old_path.exists());

		let new_path = tmp.path().join("subdir/new.txt");
		editor.rename_current_file(new_path.clone()).await.expect("rename");

		assert!(!old_path.exists(), "old file should be removed");
		assert!(new_path.exists(), "new file should exist");
		assert_eq!(std::fs::read_to_string(&new_path).unwrap(), "hello\n");

		let buf_path = editor.buffer().path().map(|p| p.to_path_buf());
		assert_eq!(buf_path, Some(new_path), "buffer path should be updated");
	}

	#[tokio::test]
	async fn rename_current_file_rejects_modified_buffer() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let old_path = tmp.path().join("mod.txt");
		std::fs::write(&old_path, "original\n").expect("write");

		let mut editor = Editor::new(old_path.clone()).await.expect("open");
		let _ = editor.buffer_mut().set_modified(true);

		let new_path = tmp.path().join("moved.txt");
		let result = editor.rename_current_file(new_path).await;
		assert!(result.is_err(), "should reject modified buffer");
		assert!(old_path.exists(), "old file should remain");
	}

	#[tokio::test]
	async fn rename_current_file_noop_for_same_path() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("same.txt");
		std::fs::write(&path, "content\n").expect("write");

		let mut editor = Editor::new(path.clone()).await.expect("open");
		editor.rename_current_file(path.clone()).await.expect("noop rename");
		assert!(path.exists());
	}

	#[tokio::test]
	async fn create_file_creates_on_disk_and_opens() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("subdir/new_file.txt");

		let mut editor = Editor::new_scratch();
		editor.create_file(path.clone()).await.expect("create");

		assert!(path.exists(), "file should be created on disk");
		assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
	}

	#[tokio::test]
	async fn create_file_opens_existing_without_overwrite() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("existing.txt");
		std::fs::write(&path, "keep me\n").expect("write");

		let mut editor = Editor::new_scratch();
		editor.create_file(path.clone()).await.expect("create existing");

		assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me\n", "existing content must be preserved");
	}

	#[tokio::test]
	async fn delete_current_file_removes_from_disk() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("delete_me.txt");
		std::fs::write(&path, "goodbye\n").expect("write");

		let mut editor = Editor::new(path.clone()).await.expect("open");
		assert!(path.exists());

		editor.delete_current_file().await.expect("delete");
		assert!(!path.exists(), "file should be deleted from disk");
	}

	#[tokio::test]
	async fn delete_current_file_rejects_modified_buffer() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("modified_delete.txt");
		std::fs::write(&path, "data\n").expect("write");

		let mut editor = Editor::new(path.clone()).await.expect("open");
		let _ = editor.buffer_mut().set_modified(true);

		let result = editor.delete_current_file().await;
		assert!(result.is_err(), "should reject modified buffer");
		assert!(path.exists(), "file should remain on disk");
	}

	#[tokio::test]
	async fn create_dir_creates_on_disk() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("new_dir/nested");

		let mut editor = Editor::new_scratch();
		editor.create_dir(path.clone()).await.expect("mkdir");

		assert!(path.is_dir(), "directory should be created on disk");
	}

	#[tokio::test]
	async fn delete_dir_removes_empty_dir() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("empty_dir");
		std::fs::create_dir(&path).expect("mkdir");

		let mut editor = Editor::new_scratch();
		editor.delete_dir(path.clone()).await.expect("rmdir");

		assert!(!path.exists(), "directory should be removed from disk");
	}

	#[tokio::test]
	async fn delete_dir_rejects_non_empty() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("non_empty_dir");
		std::fs::create_dir(&path).expect("mkdir");
		std::fs::write(path.join("file.txt"), "content").expect("write");

		let mut editor = Editor::new_scratch();
		let result = editor.delete_dir(path.clone()).await;
		assert!(result.is_err(), "should reject non-empty directory");
		assert!(path.is_dir(), "directory should remain");
	}

	#[tokio::test]
	async fn delete_dir_rejects_non_directory() {
		let tmp = tempfile::tempdir().expect("temp dir");
		let path = tmp.path().join("not_a_dir.txt");
		std::fs::write(&path, "file").expect("write");

		let mut editor = Editor::new_scratch();
		let result = editor.delete_dir(path.clone()).await;
		assert!(result.is_err(), "should reject non-directory path");
	}

	#[cfg(unix)]
	#[test]
	fn is_cross_device_rename_detects_exdev() {
		let exdev = std::io::Error::from_raw_os_error(18); // EXDEV
		assert!(Editor::is_cross_device_rename(&exdev));

		let enoent = std::io::Error::from_raw_os_error(2); // ENOENT
		assert!(!Editor::is_cross_device_rename(&enoent));
	}
}
