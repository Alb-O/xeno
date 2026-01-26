//! Editor lifecycle operations.

mod ops;
mod state;

use std::path::PathBuf;

use xeno_registry::{HookContext, HookEventData, emit_sync_with as emit_hook_sync_with};

use super::Editor;

impl Editor {
	/// Initializes the UI layer at editor startup.
	pub fn ui_startup(&mut self) {
		let mut ui = std::mem::take(&mut self.state.ui);
		ui.startup();
		self.state.ui = ui;
		self.state.frame.needs_redraw = true;
	}

	/// Ticks the UI layer, allowing it to update and request redraws.
	pub fn ui_tick(&mut self) {
		let mut ui = std::mem::take(&mut self.state.ui);
		ui.tick(self);
		if ui.take_wants_redraw() {
			self.state.frame.needs_redraw = true;
		}
		self.state.ui = ui;
	}

	/// Runs the main editor tick: dirty buffer hooks, LSP sync, and animations.
	pub fn tick(&mut self) {
		if self.state.layout.animation_needs_redraw() {
			self.state.frame.needs_redraw = true;
		}

		#[cfg(feature = "lsp")]
		if !self.state.lsp.poll_diagnostics().is_empty() {
			self.state.frame.needs_redraw = true;
		}
		#[cfg(feature = "lsp")]
		self.drain_lsp_ui_events();

		#[cfg(feature = "lsp")]
		self.queue_lsp_resyncs_from_documents();

		let dirty_ids: Vec<_> = self.state.frame.dirty_buffers.drain().collect();
		for buffer_id in dirty_ids {
			if let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) {
				let scratch_path = PathBuf::from("[scratch]");
				let path = buffer.path().unwrap_or_else(|| scratch_path.clone());
				let file_type = buffer.file_type();
				let version = buffer.version();
				let content = buffer.with_doc(|doc| doc.content().clone());
				emit_hook_sync_with(
					&HookContext::new(HookEventData::BufferChange {
						path: &path,
						text: content.slice(..),
						file_type: file_type.as_deref(),
						version,
					}),
					&mut self.state.hook_runtime,
				);
			}
		}

		#[cfg(feature = "lsp")]
		self.tick_lsp_sync();

		emit_hook_sync_with(
			&HookContext::new(HookEventData::EditorTick),
			&mut self.state.hook_runtime,
		);
	}

	/// Returns true if any UI panel is currently open.
	pub fn any_panel_open(&self) -> bool {
		self.state.ui.any_panel_open()
	}

	/// Handles terminal window resize events, updating buffer text widths and emitting hooks.
	pub fn handle_window_resize(&mut self, width: u16, height: u16) {
		self.state.viewport.width = Some(width);
		self.state.viewport.height = Some(height);

		for buffer in self.state.core.buffers.buffers_mut() {
			buffer.text_width = width.saturating_sub(buffer.gutter_width()) as usize;
		}

		let mut ui = std::mem::take(&mut self.state.ui);
		ui.notify_resize(self, width, height);
		if ui.take_wants_redraw() {
			self.state.frame.needs_redraw = true;
		}
		self.state.ui = ui;
		self.state.frame.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(HookEventData::WindowResize { width, height }),
			&mut self.state.hook_runtime,
		);
	}

	/// Handles terminal focus gained events, emitting the FocusGained hook.
	pub fn handle_focus_in(&mut self) {
		self.state.frame.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(HookEventData::FocusGained),
			&mut self.state.hook_runtime,
		);
	}

	/// Handles terminal focus lost events, emitting the FocusLost hook.
	pub fn handle_focus_out(&mut self) {
		self.state.frame.needs_redraw = true;
		emit_hook_sync_with(
			&HookContext::new(HookEventData::FocusLost),
			&mut self.state.hook_runtime,
		);
	}

	/// Handles paste events, delegating to UI or inserting text directly.
	pub fn handle_paste(&mut self, content: String) {
		let mut ui = std::mem::take(&mut self.state.ui);
		let handled = ui.handle_paste(self, content.clone());
		if ui.take_wants_redraw() {
			self.state.frame.needs_redraw = true;
		}
		self.state.ui = ui;
		self.sync_focus_from_ui();

		if handled {
			self.state.frame.needs_redraw = true;
			return;
		}

		self.insert_text(&content);
	}

	/// Emits current statistics as a tracing event.
	pub fn emit_stats(&self) {
		self.stats_snapshot().emit();
	}
}
