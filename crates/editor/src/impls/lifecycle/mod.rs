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
		let mut ui = std::mem::take(&mut self.state.ui.ui);
		ui.startup();
		self.state.ui.ui = ui;
		self.state.core.frame.needs_redraw = true;
	}

	/// Ticks the UI layer and advances the frame clock.
	///
	/// All time-based UI state progression (animations) is driven
	/// from here rather than from the render path, so skipping a render frame
	/// does not freeze or jump animations.
	pub fn ui_tick(&mut self) {
		let now = SystemTime::now();
		let _delta = now.duration_since(self.state.core.frame.last_tick).unwrap_or(Duration::from_millis(16));
		self.state.core.frame.last_tick = now;

		let mut ui = std::mem::take(&mut self.state.ui.ui);
		ui.tick(self);
		if ui.take_wants_redraw() {
			self.state.core.frame.needs_redraw = true;
		}
		self.state.ui.ui = ui;
	}

	/// Runs the main editor tick: dirty buffer hooks, LSP sync, and animations.
	///
	/// Also drains completed background syntax parses from the [`xeno_syntax::SyntaxManager`]
	/// and requests a redraw if any results were installed.
	pub fn tick(&mut self) {
		if self.state.integration.syntax_manager.drain_finished_inflight() {
			self.state.runtime.effects.request_redraw();
		}

		if self.state.core.layout.animation_needs_redraw() {
			self.state.runtime.effects.request_redraw();
		}

		#[cfg(feature = "lsp")]
		if !self.state.integration.lsp.poll_diagnostics().is_empty() {
			self.state.runtime.effects.request_redraw();
		}
		#[cfg(feature = "lsp")]
		self.drain_lsp_ui_events();
		#[cfg(feature = "lsp")]
		self.drain_lsp_apply_edits();

		#[cfg(feature = "lsp")]
		self.queue_lsp_resyncs_from_documents();

		// Emit BufferChange hooks for all modified buffers
		let dirty_ids: Vec<_> = self.state.core.frame.dirty_buffers.drain().collect();
		let scratch_path = PathBuf::from("[scratch]");

		for buffer_id in dirty_ids {
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
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
				&mut self.state.integration.work_scheduler,
			);
		}

		#[cfg(feature = "lsp")]
		self.tick_lsp_sync();
		#[cfg(feature = "lsp")]
		self.tick_inlay_hints();
		#[cfg(feature = "lsp")]
		self.tick_pull_diagnostics();
		#[cfg(feature = "lsp")]
		self.tick_semantic_tokens();
		#[cfg(feature = "lsp")]
		self.tick_document_highlights();

		emit_hook_sync_with(&HookContext::new(HookEventData::EditorTick), &mut self.state.integration.work_scheduler);

		self.flush_effects();
	}

	/// Returns true if any UI panel is currently open.
	pub fn any_panel_open(&self) -> bool {
		self.state.ui.ui.any_panel_open()
	}

	/// Handles frontend viewport resize events expressed in text-grid cells.
	pub fn handle_window_resize(&mut self, cols: u16, rows: u16) {
		self.state.core.viewport.width = Some(cols);
		self.state.core.viewport.height = Some(rows);

		for buffer in self.state.core.editor.buffers.buffers_mut() {
			buffer.text_width = cols.saturating_sub(buffer.gutter_width()) as usize;
		}

		let mut ui = std::mem::take(&mut self.state.ui.ui);
		ui.notify_resize(self, cols, rows);
		if ui.take_wants_redraw() {
			self.state.runtime.effects.request_redraw();
		}
		self.state.ui.ui = ui;

		let mut interaction = self.state.ui.overlay_system.take_interaction();
		interaction.on_viewport_changed(self);
		self.state.ui.overlay_system.restore_interaction(interaction);

		self.state.runtime.effects.request_redraw();
		emit_hook_sync_with(
			&HookContext::new(HookEventData::WindowResize { width: cols, height: rows }),
			&mut self.state.integration.work_scheduler,
		);
		self.flush_effects();
	}

	/// Handles terminal focus gained events, emitting the FocusGained hook.
	pub fn handle_focus_in(&mut self) {
		self.state.runtime.effects.request_redraw();
		emit_hook_sync_with(&HookContext::new(HookEventData::FocusGained), &mut self.state.integration.work_scheduler);
		self.flush_effects();
	}

	/// Handles terminal focus lost events, emitting the FocusLost hook.
	pub fn handle_focus_out(&mut self) {
		self.state.runtime.effects.request_redraw();
		emit_hook_sync_with(&HookContext::new(HookEventData::FocusLost), &mut self.state.integration.work_scheduler);
		self.flush_effects();
	}

	/// Handles paste events, delegating to UI or inserting text directly.
	pub fn handle_paste(&mut self, content: String) {
		let content = normalize_to_lf(content);
		let mut ui = std::mem::take(&mut self.state.ui.ui);
		let handled = if ui.focused_panel_id().is_some() {
			ui.handle_paste(self, content.clone())
		} else {
			false
		};
		if ui.take_wants_redraw() {
			self.state.runtime.effects.request_redraw();
		}
		self.state.ui.ui = ui;
		self.sync_focus_from_ui();

		if handled {
			self.state.runtime.effects.request_redraw();
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
