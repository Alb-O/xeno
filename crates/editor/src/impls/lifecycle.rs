//! Editor lifecycle operations.
//!
//! Tick, startup, and render update methods.

#[cfg(feature = "lsp")]
use std::collections::HashSet;
use std::path::PathBuf;

#[cfg(feature = "lsp")]
use futures::channel::oneshot;
#[cfg(feature = "lsp")]
use tracing::{debug, warn};
use xeno_registry::{HookContext, HookEventData, emit_sync_with as emit_hook_sync_with};

use super::Editor;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

impl Editor {
	/// Initializes the UI layer at editor startup.
	pub fn ui_startup(&mut self) {
		let mut ui = std::mem::take(&mut self.ui);
		ui.startup();
		self.ui = ui;
		self.frame.needs_redraw = true;
	}

	/// Ticks the UI layer, allowing it to update and request redraws.
	pub fn ui_tick(&mut self) {
		let mut ui = std::mem::take(&mut self.ui);
		ui.tick(self);
		if ui.take_wants_redraw() {
			self.frame.needs_redraw = true;
		}
		self.ui = ui;
	}

	/// Runs the main editor tick: dirty buffer hooks, LSP sync, and animations.
	pub fn tick(&mut self) {
		// Check if separator animation needs continuous redraws
		if self.layout.animation_needs_redraw() {
			self.frame.needs_redraw = true;
		}

		#[cfg(feature = "lsp")]
		if !self.lsp.poll_diagnostics().is_empty() {
			self.frame.needs_redraw = true;
		}
		#[cfg(feature = "lsp")]
		self.drain_lsp_ui_events();

		#[cfg(feature = "lsp")]
		let mut lsp_docs: HashSet<crate::buffer::DocumentId> = HashSet::new();

		let dirty_ids: Vec<_> = self.frame.dirty_buffers.drain().collect();
		for buffer_id in dirty_ids {
			if let Some(buffer) = self.core.buffers.get_buffer(buffer_id) {
				let scratch_path = PathBuf::from("[scratch]");
				let path = buffer.path().unwrap_or_else(|| scratch_path.clone());
				let file_type = buffer.file_type();
				let version = buffer.version();
				let content = buffer.with_doc(|doc| doc.content().clone());
				emit_hook_sync_with(
					&HookContext::new(
						HookEventData::BufferChange {
							path: &path,
							text: content.slice(..),
							file_type: file_type.as_deref(),
							version,
						},
						Some(&self.extensions),
					),
					&mut self.hook_runtime,
				);

				#[cfg(feature = "lsp")]
				if lsp_docs.insert(buffer.document_id()) {
					self.queue_lsp_change(buffer_id);
				}
			}
		}
		emit_hook_sync_with(
			&HookContext::new(HookEventData::EditorTick, Some(&self.extensions)),
			&mut self.hook_runtime,
		);
	}

	/// Marks a buffer dirty for LSP full sync (clears incremental changes, bumps version).
	///
	/// Use this after operations that replace the entire document content (e.g., undo/redo)
	/// where incremental sync is not possible.
	pub(crate) fn mark_buffer_dirty_for_full_sync(&mut self, buffer_id: crate::buffer::BufferId) {
		if let Some(buffer) = self.core.buffers.get_buffer_mut(buffer_id) {
			buffer.with_doc_mut(|doc| {
				doc.increment_version();
				#[cfg(feature = "lsp")]
				doc.mark_for_full_lsp_sync();
			});
		}
		self.frame.dirty_buffers.insert(buffer_id);
	}

	/// Maximum number of incremental changes before falling back to full sync.
	#[cfg(feature = "lsp")]
	const LSP_MAX_INCREMENTAL_CHANGES: usize = 100;

	/// Maximum total bytes of inserted text before falling back to full sync.
	#[cfg(feature = "lsp")]
	const LSP_MAX_INCREMENTAL_BYTES: usize = 100 * 1024; // 100 KB

	/// Queues an LSP buffer change notification to be processed asynchronously.
	#[cfg(feature = "lsp")]
	fn queue_lsp_change(&mut self, buffer_id: crate::buffer::BufferId) {
		let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		let (Some(path), Some(language)) = (buffer.path(), buffer.file_type()) else {
			return;
		};
		let (force_full_sync, has_pending) =
			buffer.with_doc(|doc| (doc.needs_full_lsp_sync(), doc.has_pending_lsp_sync()));
		if !has_pending {
			return;
		}
		let content = buffer.with_doc(|doc| doc.content().clone());
		let changes = buffer.drain_lsp_changes();
		if force_full_sync {
			buffer.with_doc_mut(|doc| doc.clear_full_lsp_sync());
		}
		let supports_incremental = self.lsp.incremental_encoding_for_buffer(buffer).is_some();

		// Safety fallback: skip incremental if too many changes or too much data
		let change_count = changes.len();
		let total_bytes: usize = changes.iter().map(|c| c.new_text.len()).sum();
		let use_incremental = !force_full_sync
			&& supports_incremental
			&& !changes.is_empty()
			&& change_count <= Self::LSP_MAX_INCREMENTAL_CHANGES
			&& total_bytes <= Self::LSP_MAX_INCREMENTAL_BYTES;

		debug!(
			path = ?path,
			mode = if use_incremental { "incremental" } else { "full" },
			change_count,
			total_bytes,
			supports_incremental,
			force_full_sync,
			"LSP sync mode selected"
		);

		let sync = self.lsp.sync().clone();
		tokio::spawn(async move {
			let result = if use_incremental {
				sync.notify_change_incremental(&path, &language, &content, changes)
					.await
			} else {
				sync.notify_change_full(&path, &language, &content).await
			};
			if let Err(e) = result {
				warn!(error = %e, path = ?path, "LSP change notification failed");
			}
		});
	}

	/// Queues an immediate LSP change and returns an ack receiver when written.
	#[cfg(feature = "lsp")]
	fn queue_lsp_change_immediate(
		&mut self,
		buffer_id: crate::buffer::BufferId,
	) -> Option<oneshot::Receiver<()>> {
		let buffer = self.core.buffers.get_buffer(buffer_id)?;
		let (Some(path), Some(language)) = (buffer.path(), buffer.file_type()) else {
			return None;
		};
		let (force_full_sync, has_pending) =
			buffer.with_doc(|doc| (doc.needs_full_lsp_sync(), doc.has_pending_lsp_sync()));
		if !has_pending {
			return None;
		}
		let content = buffer.with_doc(|doc| doc.content().clone());
		let changes = buffer.drain_lsp_changes();
		if force_full_sync {
			buffer.with_doc_mut(|doc| doc.clear_full_lsp_sync());
		}
		let supports_incremental = self.lsp.incremental_encoding_for_buffer(buffer).is_some();
		let change_count = changes.len();
		let total_bytes: usize = changes.iter().map(|c| c.new_text.len()).sum();
		let use_incremental = !force_full_sync
			&& supports_incremental
			&& !changes.is_empty()
			&& change_count <= Self::LSP_MAX_INCREMENTAL_CHANGES
			&& total_bytes <= Self::LSP_MAX_INCREMENTAL_BYTES;

		let sync = self.lsp.sync().clone();
		let (tx, rx) = oneshot::channel();
		tokio::spawn(async move {
			let result = if use_incremental {
				sync.notify_change_incremental_with_ack(&path, &language, &content, changes)
					.await
			} else {
				sync.notify_change_full_with_ack(&path, &language, &content)
					.await
			};
			match result {
				Ok(Some(ack)) => {
					let _ = ack.await;
				}
				Ok(None) => {}
				Err(e) => {
					warn!(error = %e, path = ?path, "LSP immediate change failed");
				}
			}
			let _ = tx.send(());
		});

		Some(rx)
	}

	/// Immediately flush LSP changes for specified buffers.
	#[cfg(feature = "lsp")]
	pub fn flush_lsp_sync_now(&mut self, buffer_ids: &[crate::buffer::BufferId]) -> FlushHandle {
		let mut handles = Vec::new();
		for &buffer_id in buffer_ids {
			if let Some(handle) = self.queue_lsp_change_immediate(buffer_id) {
				handles.push(handle);
			}
		}
		FlushHandle { handles }
	}

	/// Clears and updates style overlays (called before each render frame).
	pub fn update_style_overlays(&mut self) {
		self.style_overlays.clear();
		if self.style_overlays.has_animations() {
			self.frame.needs_redraw = true;
		}
	}

	/// Returns true if any UI panel is currently open.
	pub fn any_panel_open(&self) -> bool {
		self.ui.any_panel_open()
	}

	/// Handles terminal window resize events, updating buffer text widths and emitting hooks.
	pub fn handle_window_resize(&mut self, width: u16, height: u16) {
		self.viewport.width = Some(width);
		self.viewport.height = Some(height);

		// Update text width for all buffers
		for buffer in self.core.buffers.buffers_mut() {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		let mut ui = std::mem::take(&mut self.ui);
		ui.notify_resize(self, width, height);
		if ui.take_wants_redraw() {
			self.frame.needs_redraw = true;
		}
		self.ui = ui;
		self.frame.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::WindowResize { width, height },
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
	}

	/// Handles terminal focus gained events, emitting the FocusGained hook.
	pub fn handle_focus_in(&mut self) {
		self.frame.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(HookEventData::FocusGained, Some(&self.extensions)),
			&mut self.hook_runtime,
		);
	}

	/// Handles terminal focus lost events, emitting the FocusLost hook.
	pub fn handle_focus_out(&mut self) {
		self.frame.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(HookEventData::FocusLost, Some(&self.extensions)),
			&mut self.hook_runtime,
		);
	}

	/// Handles paste events, delegating to UI or inserting text directly.
	pub fn handle_paste(&mut self, content: String) {
		let mut ui = std::mem::take(&mut self.ui);
		let handled = ui.handle_paste(self, content.clone());
		if ui.take_wants_redraw() {
			self.frame.needs_redraw = true;
		}
		self.ui = ui;
		self.sync_focus_from_ui();

		if handled {
			self.frame.needs_redraw = true;
			return;
		}

		self.insert_text(&content);
	}

	/// Drains and executes all queued commands.
	///
	/// Routes commands through [`run_invocation`] for consistent capability
	/// checking and hook emission. Tries editor-direct commands first, then
	/// falls back to registry commands.
	///
	/// Returns `true` if any command requested quit.
	pub async fn drain_command_queue(&mut self) -> bool {
		let commands: Vec<_> = self.core.workspace.command_queue.drain().collect();

		// Use log-only mode for now (Phase 6 migration)
		let policy = InvocationPolicy::log_only();

		for cmd in commands {
			let args: Vec<String> = cmd.args.iter().map(|s| s.to_string()).collect();

			// Try editor command first
			let invocation = Invocation::EditorCommand {
				name: cmd.name.to_string(),
				args: args.clone(),
			};

			let result = self.run_invocation(invocation, policy).await;

			match result {
				InvocationResult::NotFound(_) => {
					// Not an editor command, try registry command
					let invocation = Invocation::Command {
						name: cmd.name.to_string(),
						args,
					};

					let result = self.run_invocation(invocation, policy).await;

					match result {
						InvocationResult::NotFound(_) => {
							self.show_notification(
								xeno_registry_notifications::keys::unknown_command::call(cmd.name),
							);
						}
						InvocationResult::Quit | InvocationResult::ForceQuit => return true,
						_ => {}
					}
				}
				InvocationResult::Quit | InvocationResult::ForceQuit => return true,
				_ => {}
			}
		}
		false
	}
}

#[cfg(feature = "lsp")]
pub struct FlushHandle {
	handles: Vec<oneshot::Receiver<()>>,
}

#[cfg(feature = "lsp")]
impl FlushHandle {
	/// Wait until all didChange messages have been written.
	pub async fn await_synced(self) {
		for handle in self.handles {
			let _ = handle.await;
		}
	}
}
