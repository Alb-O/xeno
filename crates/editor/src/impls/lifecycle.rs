//! Editor lifecycle operations.
//!
//! Tick, startup, and render update methods.

#[cfg(feature = "lsp")]
use std::collections::HashSet;
use std::path::PathBuf;
#[cfg(feature = "lsp")]
use std::time::Instant;

#[cfg(feature = "lsp")]
use futures::channel::oneshot;
#[cfg(feature = "lsp")]
use tracing::warn;
use xeno_registry::{HookContext, HookEventData, emit_sync_with as emit_hook_sync_with};

use super::Editor;
#[cfg(feature = "lsp")]
use crate::lsp::pending::{
	LSP_DEBOUNCE, LSP_MAX_INCREMENTAL_BYTES, LSP_MAX_INCREMENTAL_CHANGES,
};
use crate::metrics::StatsSnapshot;
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
					self.accumulate_lsp_change(buffer_id);
				}
			}
		}

		#[cfg(feature = "lsp")]
		self.flush_lsp_pending();

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

	/// Accumulates LSP buffer changes for debounced sync.
	///
	/// Instead of immediately sending notifications, changes are accumulated
	/// in [`PendingLspState`] and flushed after the debounce period elapses.
	#[cfg(feature = "lsp")]
	fn accumulate_lsp_change(&mut self, buffer_id: crate::buffer::BufferId) {
		let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		let (Some(path), Some(language)) = (buffer.path(), buffer.file_type()) else {
			return;
		};
		let (force_full_sync, has_pending, editor_version) = buffer.with_doc(|doc| {
			(
				doc.needs_full_lsp_sync(),
				doc.has_pending_lsp_sync(),
				doc.version(),
			)
		});
		if !has_pending {
			return;
		}
		let doc_id = buffer.document_id();
		let changes = buffer.drain_lsp_changes();
		if force_full_sync {
			buffer.with_doc_mut(|doc| doc.clear_full_lsp_sync());
		}

		let supports_incremental = self.lsp.incremental_encoding_for_buffer(buffer).is_some();
		let encoding = self.lsp.offset_encoding_for_buffer(buffer);

		self.pending_lsp.accumulate(
			doc_id,
			path,
			language,
			changes,
			force_full_sync,
			supports_incremental,
			encoding,
			editor_version,
		);
	}

	/// Flushes pending LSP changes that have exceeded the debounce period.
	#[cfg(feature = "lsp")]
	fn flush_lsp_pending(&mut self) {
		let now = Instant::now();
		let sync = self.lsp.sync().clone();
		let buffers = &self.core.buffers;
		let metrics = &self.metrics;

		self.pending_lsp.flush_due(now, LSP_DEBOUNCE, &sync, metrics, |doc_id| {
			buffers
				.buffers()
				.find(|b| b.document_id() == doc_id)
				.map(|b| b.with_doc(|doc| doc.content().clone()))
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
			&& change_count <= LSP_MAX_INCREMENTAL_CHANGES
			&& total_bytes <= LSP_MAX_INCREMENTAL_BYTES;

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
			let invocation = Invocation::EditorCommand {
				name: cmd.name.to_string(),
				args: args.clone(),
			};

			let result = self.run_invocation(invocation, policy).await;
			match result {
				InvocationResult::NotFound(_) => {
					let invocation = Invocation::Command {
						name: cmd.name.to_string(),
						args,
					};
					match self.run_invocation(invocation, policy).await {
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

	/// Collects a snapshot of current editor statistics.
	pub fn stats_snapshot(&self) -> StatsSnapshot {
		#[cfg(feature = "lsp")]
		let (lsp_pending_docs, lsp_in_flight) = (
			self.pending_lsp.pending_count(),
			self.pending_lsp.in_flight_count(),
		);
		#[cfg(not(feature = "lsp"))]
		let (lsp_pending_docs, lsp_in_flight) = (0, 0);

		StatsSnapshot {
			hooks_pending: self.hook_runtime.pending_count(),
			hooks_scheduled: self.hook_runtime.scheduled_total(),
			hooks_completed: self.hook_runtime.completed_total(),
			lsp_pending_docs,
			lsp_in_flight,
			lsp_full_sync: self.metrics.full_sync_count(),
			lsp_incremental_sync: self.metrics.incremental_sync_count(),
			lsp_send_errors: self.metrics.send_error_count(),
			lsp_coalesced: self.metrics.coalesced_count(),
		}
	}

	/// Emits current statistics as a tracing event.
	pub fn emit_stats(&self) {
		self.stats_snapshot().emit();
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
