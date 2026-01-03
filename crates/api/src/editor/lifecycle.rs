//! Editor lifecycle operations.
//!
//! Tick, startup, and render update methods.

use std::path::PathBuf;

use xeno_registry::commands::{CommandContext, CommandOutcome, find_command};
use xeno_registry::{HookContext, HookEventData, emit_sync_with as emit_hook_sync_with};

use super::Editor;
use super::extensions::{RENDER_EXTENSIONS, TICK_EXTENSIONS};

impl Editor {
	/// Initializes the UI layer at editor startup.
	pub fn ui_startup(&mut self) {
		let mut ui = std::mem::take(&mut self.ui);
		ui.startup();
		self.ui = ui;
		self.needs_redraw = true;
	}

	/// Ticks the UI layer, allowing it to update and request redraws.
	pub fn ui_tick(&mut self) {
		let mut ui = std::mem::take(&mut self.ui);
		ui.tick(self);
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;
	}

	/// Runs the main editor tick: extensions, dirty buffer hooks, and animations.
	pub fn tick(&mut self) {
		let mut sorted_ticks: Vec<_> = TICK_EXTENSIONS.iter().collect();
		sorted_ticks.sort_by_key(|e| e.priority);
		for ext in sorted_ticks {
			(ext.tick)(self);
		}

		// Check if separator animation needs continuous redraws
		if self.layout.animation_needs_redraw() {
			self.needs_redraw = true;
		}

		let dirty_ids: Vec<_> = self.dirty_buffers.drain().collect();
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
			}
		}
		emit_hook_sync_with(
			&HookContext::new(HookEventData::EditorTick, Some(&self.extensions)),
			&mut self.hook_runtime,
		);
	}

	/// Updates style overlays from render extensions (called before each render frame).
	pub fn update_style_overlays(&mut self) {
		self.style_overlays.clear();

		let mut sorted: Vec<_> = RENDER_EXTENSIONS.iter().collect();
		sorted.sort_by_key(|e| e.priority);
		for ext in sorted {
			(ext.update)(self);
		}

		if self.style_overlays.has_animations() {
			self.needs_redraw = true;
		}
	}

	/// Returns true if any UI panel is currently open.
	pub fn any_panel_open(&self) -> bool {
		self.ui.any_panel_open()
	}

	/// Handles terminal window resize events, updating buffer text widths and emitting hooks.
	pub fn handle_window_resize(&mut self, width: u16, height: u16) {
		self.window_width = Some(width);
		self.window_height = Some(height);

		// Update text width for all buffers
		for buffer in self.buffers.buffers_mut() {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		let mut ui = std::mem::take(&mut self.ui);
		ui.notify_resize(self, width, height);
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;
		self.needs_redraw = true;
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
		self.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(HookEventData::FocusGained, Some(&self.extensions)),
			&mut self.hook_runtime,
		);
	}

	/// Handles terminal focus lost events, emitting the FocusLost hook.
	pub fn handle_focus_out(&mut self) {
		self.needs_redraw = true;
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
			self.needs_redraw = true;
		}
		self.ui = ui;

		if handled {
			self.needs_redraw = true;
			return;
		}

		self.insert_text(&content);
	}

	/// Drains and executes all queued commands.
	///
	/// Commands are queued when actions return [`ActionResult::Command`]. This
	/// method should be called each tick after processing input events.
	///
	/// Returns `true` if any command requested quit.
	pub async fn drain_command_queue(&mut self) -> bool {
		let commands: Vec<_> = self.command_queue.drain().collect();
		for cmd in commands {
			let Some(command_def) = find_command(cmd.name) else {
				self.show_notification(xeno_registry_notifications::keys::unknown_command::call(
					cmd.name,
				));
				continue;
			};

			let args: Vec<&str> = cmd.args.iter().map(|s| s.as_str()).collect();
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
		let buffer_id = self.buffers.focused_view();
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
