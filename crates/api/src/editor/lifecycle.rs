//! Editor lifecycle operations.
//!
//! Tick, startup, and render update methods.

#[cfg(feature = "lsp")]
use std::collections::HashSet;
use std::path::PathBuf;

use tracing::{debug, warn};
use xeno_registry::commands::{CommandContext, CommandOutcome, find_command};
use xeno_registry::{HookContext, HookEventData, emit_sync_with as emit_hook_sync_with};

use super::Editor;
use crate::commands::{EditorCommandContext, find_editor_command};

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
		let mut lsp_docs: HashSet<crate::buffer::DocumentId> = HashSet::new();

		let dirty_ids: Vec<_> = self.frame.dirty_buffers.drain().collect();
		for buffer_id in dirty_ids {
			if let Some(buffer) = self.buffers.get_buffer(buffer_id) {
				let scratch_path = PathBuf::from("[scratch]");
				let path = buffer.path().unwrap_or_else(|| scratch_path.clone());
				let file_type = buffer.file_type();
				let version = buffer.version();
				let content = buffer.doc().content.clone();
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
		if let Some(buffer) = self.buffers.get_buffer_mut(buffer_id) {
			let mut doc = buffer.doc_mut();
			doc.version = doc.version.wrapping_add(1);
			#[cfg(feature = "lsp")]
			{
				doc.pending_lsp_changes.clear();
			}
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
		let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
			return;
		};
		let (Some(path), Some(language)) = (buffer.path(), buffer.file_type()) else {
			return;
		};
		let content = buffer.doc().content.clone();
		let changes = buffer.drain_lsp_changes();
		let supports_incremental = self.lsp.incremental_encoding_for_buffer(buffer).is_some();

		// Safety fallback: skip incremental if too many changes or too much data
		let change_count = changes.len();
		let total_bytes: usize = changes.iter().map(|c| c.new_text.len()).sum();
		let use_incremental = supports_incremental
			&& !changes.is_empty()
			&& change_count <= Self::LSP_MAX_INCREMENTAL_CHANGES
			&& total_bytes <= Self::LSP_MAX_INCREMENTAL_BYTES;

		debug!(
			path = ?path,
			mode = if use_incremental { "incremental" } else { "full" },
			change_count,
			total_bytes,
			supports_incremental,
			"LSP sync mode selected"
		);

		let sync = self.lsp.sync().clone();
		tokio::spawn(async move {
			let result = if use_incremental {
				sync.notify_change_incremental_v2(&path, &language, &content, changes)
					.await
			} else {
				sync.notify_change_full(&path, &language, &content).await
			};
			if let Err(e) = result {
				warn!(error = %e, path = ?path, "LSP change notification failed");
			}
		});
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
		for buffer in self.buffers.buffers_mut() {
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
	/// Checks [`EDITOR_COMMANDS`] first, then [`COMMANDS`].
	/// Returns `true` if any command requested quit.
	pub async fn drain_command_queue(&mut self) -> bool {
		let commands: Vec<_> = self.workspace.command_queue.drain().collect();
		for cmd in commands {
			let args: Vec<&str> = cmd.args.iter().map(|s| s.as_str()).collect();

			if let Some(editor_cmd) = find_editor_command(cmd.name) {
				let mut ctx = EditorCommandContext {
					editor: self,
					args: &args,
					count: 1,
					register: None,
					user_data: editor_cmd.user_data,
				};
				match (editor_cmd.handler)(&mut ctx).await {
					Ok(CommandOutcome::Ok) => {}
					Ok(CommandOutcome::Quit | CommandOutcome::ForceQuit) => return true,
					Err(e) => {
						self.show_notification(
							xeno_registry_notifications::keys::command_error::call(&e.to_string()),
						);
					}
				}
				continue;
			}

			let Some(command_def) = find_command(cmd.name) else {
				self.show_notification(xeno_registry_notifications::keys::unknown_command::call(
					cmd.name,
				));
				continue;
			};
			let mut ctx = CommandContext {
				editor: self,
				args: &args,
				count: 1,
				register: None,
				user_data: command_def.user_data,
			};
			match (command_def.handler)(&mut ctx).await {
				Ok(CommandOutcome::Ok) => {}
				Ok(CommandOutcome::Quit | CommandOutcome::ForceQuit) => return true,
				Err(e) => {
					self.show_notification(xeno_registry_notifications::keys::command_error::call(
						&e.to_string(),
					));
				}
			}
		}
		false
	}

	/// Maps sibling buffer selections through a transaction.
	pub(super) fn sync_sibling_selections(&mut self, tx: &xeno_base::Transaction) {
		let buffer_id = self.focused_view();
		let doc_id = self
			.buffers
			.get_buffer(buffer_id)
			.expect("focused buffer must exist")
			.document_id();

		let sibling_ids: Vec<_> = self
			.buffers
			.buffer_ids()
			.filter(|&id| id != buffer_id)
			.filter(|&id| {
				self.buffers
					.get_buffer(id)
					.is_some_and(|b| b.document_id() == doc_id)
			})
			.collect();

		for sibling_id in sibling_ids {
			if let Some(sibling) = self.buffers.get_buffer_mut(sibling_id) {
				sibling.map_selection_through(tx);
			}
		}
	}
}
