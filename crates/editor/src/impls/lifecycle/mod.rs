//! Editor lifecycle operations.

mod ops;
mod state;

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use xeno_registry::HookEventData;
use xeno_registry::hooks::{HookContext, emit_sync_with as emit_hook_sync_with};

use super::Editor;
use crate::paste::normalize_to_lf;

impl Editor {
	/// Initializes the UI layer at editor startup.
	pub fn ui_startup(&mut self) {
		let mut ui = std::mem::take(&mut self.state.ui);
		ui.startup();
		self.state.ui = ui;
		self.state.frame.needs_redraw = true;
	}

	/// Ticks the UI layer and advances the frame clock.
	///
	/// All time-based state progression (notifications, animations) is driven
	/// from here rather than from the render path, so skipping a render frame
	/// does not freeze or jump animations.
	pub fn ui_tick(&mut self) {
		let now = SystemTime::now();
		let delta = now.duration_since(self.state.frame.last_tick).unwrap_or(Duration::from_millis(16));
		self.state.frame.last_tick = now;

		let had_notifications = !self.state.notifications.is_empty();
		self.state.notifications.tick(delta);
		if had_notifications {
			self.state.frame.needs_redraw = true;
		}

		let mut ui = std::mem::take(&mut self.state.ui);
		ui.tick(self);
		if ui.take_wants_redraw() {
			self.state.frame.needs_redraw = true;
		}
		self.state.ui = ui;
	}

	/// Runs the main editor tick: dirty buffer hooks, LSP sync, and animations.
	///
	/// Also drains completed background syntax parses from the [`crate::syntax_manager::SyntaxManager`]
	/// and requests a redraw if any results were installed.
	pub fn tick(&mut self) {
		if self.state.syntax_manager.drain_finished_inflight() {
			self.state.effects.request_redraw();
		}

		if self.state.layout.animation_needs_redraw() {
			self.state.effects.request_redraw();
		}

		#[cfg(feature = "lsp")]
		if !self.state.lsp.poll_diagnostics().is_empty() {
			self.state.effects.request_redraw();
		}
		#[cfg(feature = "lsp")]
		self.drain_lsp_ui_events();

		#[cfg(feature = "lsp")]
		self.queue_lsp_resyncs_from_documents();

		// Emit BufferChange hooks for all modified buffers
		let dirty_ids: Vec<_> = self.state.frame.dirty_buffers.drain().collect();
		let scratch_path = PathBuf::from("[scratch]");

		for buffer_id in dirty_ids {
			let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
				continue;
			};

			let (path, file_type, version, text) = buffer.with_doc(|doc| {
				(
					doc.path().cloned().unwrap_or_else(|| scratch_path.clone()),
					doc.file_type().map(String::from),
					doc.version(),
					doc.content().clone(),
				)
			});

			emit_hook_sync_with(
				&HookContext::new(HookEventData::BufferChange {
					path: &path,
					text: text.slice(..),
					file_type: file_type.as_deref(),
					version,
				}),
				&mut self.state.hook_runtime,
			);
		}

		#[cfg(feature = "lsp")]
		self.tick_lsp_sync();

		emit_hook_sync_with(&HookContext::new(HookEventData::EditorTick), &mut self.state.hook_runtime);

		self.flush_effects();
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
			self.state.effects.request_redraw();
		}
		self.state.ui = ui;

		let mut interaction = std::mem::take(&mut self.state.overlay_system.interaction);
		interaction.on_viewport_changed(self);
		self.state.overlay_system.interaction = interaction;

		self.state.effects.request_redraw();
		emit_hook_sync_with(&HookContext::new(HookEventData::WindowResize { width, height }), &mut self.state.hook_runtime);
		self.flush_effects();
	}

	/// Handles terminal focus gained events, emitting the FocusGained hook.
	pub fn handle_focus_in(&mut self) {
		self.state.effects.request_redraw();
		emit_hook_sync_with(&HookContext::new(HookEventData::FocusGained), &mut self.state.hook_runtime);
		self.flush_effects();
	}

	/// Handles terminal focus lost events, emitting the FocusLost hook.
	pub fn handle_focus_out(&mut self) {
		self.state.effects.request_redraw();
		emit_hook_sync_with(&HookContext::new(HookEventData::FocusLost), &mut self.state.hook_runtime);
		self.flush_effects();
	}

	/// Handles paste events, delegating to UI or inserting text directly.
	pub fn handle_paste(&mut self, content: String) {
		let content = normalize_to_lf(content);
		let mut ui = std::mem::take(&mut self.state.ui);
		let handled = if ui.focused_panel_id().is_some() {
			ui.handle_paste(self, content.clone())
		} else {
			false
		};
		if ui.take_wants_redraw() {
			self.state.effects.request_redraw();
		}
		self.state.ui = ui;
		self.sync_focus_from_ui();

		if handled {
			self.state.effects.request_redraw();
			self.flush_effects();
			return;
		}

		if !self.snippet_replace_mode_insert(&content) {
			self.paste_text(&content);
		}
		self.flush_effects();
	}

	/// Emits current statistics as a tracing event.
	pub fn emit_stats(&self) {
		self.stats_snapshot().emit();
	}
}
