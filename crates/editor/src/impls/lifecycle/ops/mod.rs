//! Lifecycle operation implementations for Editor runtime behavior.

#[cfg(feature = "lsp")]
use std::time::Instant;

#[cfg(feature = "lsp")]
use tokio::sync::oneshot;

use super::super::Editor;
#[cfg(feature = "lsp")]
use super::state::FlushHandle;
use crate::metrics::StatsSnapshot;

impl Editor {
	/// Orchestrates background syntax parsing for all buffers and installs results.
	///
	/// Dedupes parsing by document, ensuring shared documents are only processed once.
	///
	/// # Hotness and Retention
	///
	/// Non-visible documents are marked as `Cold` to allow eviction when memory
	/// or TTL thresholds are met.
	pub fn ensure_syntax_for_buffers(&mut self) {
		use std::collections::{HashMap, HashSet};

		use xeno_syntax::{EnsureSyntaxContext, SyntaxHotness};

		let loader = std::sync::Arc::clone(&self.state.config.config.language_loader);
		let mut visible_ids: HashSet<_> = self.state.core.windows.base_window().layout.views().into_iter().collect();

		if let Some(active) = self.state.ui.overlay_system.interaction().active() {
			for pane in &active.session.panes {
				visible_ids.insert(pane.buffer);
			}
		}

		if let Some(store) = self.overlays().get::<crate::info_popup::InfoPopupStore>() {
			for id in store.ids() {
				if let Some(popup) = store.get(id) {
					visible_ids.insert(popup.buffer_id);
				}
			}
		}

		let mut doc_hotness = HashMap::new();
		let mut doc_viewports = HashMap::new();

		for &view in &visible_ids {
			if let Some(buffer) = self.state.core.editor.buffers.get_buffer(view) {
				let doc_id = buffer.document_id();
				let tab_width = self.tab_width_for(view);
				let height = self.view_area(view).height;
				let gutter = buffer.gutter_width();

				let start_char = buffer.screen_to_doc_position(0, gutter, tab_width).unwrap_or(0);
				let end_char = buffer.screen_to_doc_position(height, gutter, tab_width).unwrap_or(start_char);

				let (start_byte, end_byte, doc_bytes) = buffer.with_doc(|doc| {
					let content = doc.content();
					let max_char = content.len_chars();
					let mut lo = start_char.min(max_char);
					let mut hi = end_char.min(max_char);
					if hi < lo {
						std::mem::swap(&mut lo, &mut hi);
					}

					let mut start_byte = content.char_to_byte(lo) as u32;
					let mut end_byte = content.char_to_byte(hi) as u32;
					let len_bytes = content.len_bytes() as u32;
					if end_byte < start_byte {
						std::mem::swap(&mut start_byte, &mut end_byte);
					}
					if end_byte == start_byte && start_byte < len_bytes {
						end_byte = start_byte + 1;
					}
					(start_byte, end_byte.min(len_bytes), content.len_bytes())
				});

				let span_cap = self.state.integration.syntax_manager.viewport_visible_span_cap_for_bytes(doc_bytes);
				let viewport = start_byte..end_byte.min(start_byte.saturating_add(span_cap));
				doc_viewports
					.entry(doc_id)
					.and_modify(|v: &mut std::ops::Range<u32>| {
						v.start = v.start.min(viewport.start);
						v.end = v.end.max(viewport.end);
					})
					.or_insert(viewport);
			}
		}

		for buffer in self.state.core.editor.buffers.buffers() {
			let doc_id = buffer.document_id();
			let hotness = if visible_ids.contains(&buffer.id) {
				self.state.integration.syntax_manager.note_visible_doc(doc_id);
				SyntaxHotness::Visible
			} else if self.state.integration.syntax_manager.is_warm_doc(doc_id) {
				SyntaxHotness::Warm
			} else {
				SyntaxHotness::Cold
			};

			doc_hotness
				.entry(doc_id)
				.and_modify(|h| match (hotness, *h) {
					(SyntaxHotness::Visible, _) => *h = SyntaxHotness::Visible,
					(SyntaxHotness::Warm, SyntaxHotness::Cold) => *h = SyntaxHotness::Warm,
					_ => {}
				})
				.or_insert(hotness);
		}

		// Sweep retention for cold docs that may never get polled by ensure_syntax.
		{
			let now = std::time::Instant::now();
			if self
				.state
				.integration
				.syntax_manager
				.sweep_retention(now, |doc_id| doc_hotness.get(&doc_id).copied().unwrap_or(SyntaxHotness::Cold))
			{
				self.state.runtime.effects.request_redraw();
			}
		}

		let mut workset: HashSet<_> = self
			.state
			.integration
			.syntax_manager
			.pending_docs()
			.chain(self.state.integration.syntax_manager.dirty_docs())
			.chain(self.state.integration.syntax_manager.docs_with_completed())
			.collect();

		for (&doc_id, &hotness) in &doc_hotness {
			if hotness == SyntaxHotness::Visible {
				workset.insert(doc_id);
			}
		}

		for doc_id in workset {
			// Skip cold docs that don't parse when hidden — retention sweep handles cleanup.
			let hotness = doc_hotness.get(&doc_id).copied().unwrap_or(SyntaxHotness::Cold);
			if hotness == SyntaxHotness::Cold && self.state.integration.syntax_manager.is_hidden_parse_disabled(doc_id) {
				continue;
			}

			let Some(buffer_id) = self.state.core.editor.buffers.any_buffer_for_doc(doc_id) else {
				continue;
			};
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
				continue;
			};

			let (version, lang_id, content) = buffer.with_doc(|doc| (doc.version(), doc.language_id(), doc.content().clone()));

			let hotness = doc_hotness.get(&doc_id).copied().unwrap_or(SyntaxHotness::Warm);

			let viewport = doc_viewports.get(&doc_id).cloned();

			let outcome = self.state.integration.syntax_manager.ensure_syntax(EnsureSyntaxContext {
				doc_id,
				doc_version: version,
				language_id: lang_id,
				content: &content,
				hotness,
				loader: &loader,
				viewport,
			});

			if outcome.updated {
				self.state.runtime.effects.request_redraw();
			}
		}
	}

	/// Queues full LSP syncs for documents flagged by the LSP state manager.
	#[cfg(feature = "lsp")]
	pub(super) fn queue_lsp_resyncs_from_documents(&mut self) {
		use std::collections::HashSet;

		let uris = self.state.integration.lsp.documents().take_force_full_sync_uris();
		if uris.is_empty() {
			return;
		}

		let uri_set: HashSet<&str> = uris.iter().map(|u| u.as_str()).collect();
		for buffer in self.state.core.editor.buffers.buffers() {
			let Some(path) = buffer.path() else {
				continue;
			};
			let abs_path = crate::paths::fast_abs(&path);
			let Some(uri) = xeno_lsp::uri_from_path(&abs_path) else {
				continue;
			};
			if !uri_set.contains(uri.as_str()) {
				continue;
			}

			self.state.integration.lsp.sync_manager_mut().escalate_full(buffer.document_id());
		}
	}

	/// Ticks the LSP sync manager, flushing due documents.
	#[cfg(feature = "lsp")]
	pub(super) fn tick_lsp_sync(&mut self) {
		let sync = self.state.integration.lsp.sync().clone();
		let buffers = &self.state.core.editor.buffers;
		let stats = self
			.state
			.integration
			.lsp
			.sync_manager_mut()
			.tick(Instant::now(), &sync, &self.state.telemetry.metrics, |doc_id| {
				let view_id = buffers.any_buffer_for_doc(doc_id)?;
				let buffer = buffers.get_buffer(view_id)?;
				Some(buffer.with_doc(|doc| (doc.content().clone(), doc.version())))
			});
		self.state
			.telemetry
			.metrics
			.record_lsp_tick(stats.full_syncs, stats.incremental_syncs, stats.snapshot_bytes);
	}

	/// Requests inlay hints for visible buffers whose cache is stale.
	///
	/// Only requests hints for buffers in the active layout (visible on screen).
	/// Uses in-flight tracking to prevent duplicate requests for the same buffer.
	#[cfg(feature = "lsp")]
	pub(super) fn tick_inlay_hints(&mut self) {
		// Check refresh signal first.
		if self.state.integration.lsp.sync().take_inlay_hint_refresh() {
			self.state.ui.inlay_hint_cache.invalidate_all();
		}

		let viewport_height = self.state.core.viewport.height.unwrap_or(24) as usize;
		let visible_ids = self.base_window().layout.buffer_ids();

		for &buffer_id in &visible_ids {
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
				continue;
			};
			let scroll_line = buffer.scroll_line;
			let doc_rev = buffer.version();
			let line_lo = scroll_line;
			let line_hi = scroll_line + viewport_height + 2;

			if self.state.ui.inlay_hint_cache.get(buffer_id, doc_rev, line_lo, line_hi).is_some() {
				continue;
			}

			if self.state.ui.inlay_hint_cache.is_in_flight(buffer_id) {
				continue;
			}

			let generation = self.state.ui.inlay_hint_cache.bump_generation(buffer_id);
			self.state.ui.inlay_hint_cache.mark_in_flight(buffer_id, generation);
			self.state.integration.lsp.request_inlay_hints(buffer, generation, line_lo, line_hi);
		}
	}

	/// Requests pull diagnostics for visible buffers whose servers support it.
	///
	/// Uses `PullDiagState` for in-flight tracking, doc version checks, and
	/// `previous_result_id` propagation. Failed requests are retried on next tick.
	///
	/// Results arrive via `PullDiagnosticResult` UI event and are fed into the
	/// existing diagnostics rendering pipeline.
	#[cfg(feature = "lsp")]
	pub(super) fn tick_pull_diagnostics(&mut self) {
		if self.state.integration.lsp.sync().take_diagnostic_refresh() {
			self.state.ui.pull_diag_state.invalidate_all();
		}

		let visible_ids = self.base_window().layout.buffer_ids();

		for &buffer_id in &visible_ids {
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
				continue;
			};

			let doc_rev = buffer.version();
			if !self.state.ui.pull_diag_state.needs_request(buffer_id, doc_rev) {
				continue;
			}

			let prev_id = self.state.ui.pull_diag_state.previous_result_id(buffer_id);
			self.state.ui.pull_diag_state.mark_in_flight(buffer_id);
			self.state.integration.lsp.request_pull_diagnostics(buffer, doc_rev, prev_id);
		}
	}

	/// Requests semantic tokens for visible buffers whose cache is stale.
	///
	/// Mirrors the inlay hints tick: checks refresh signal, iterates visible
	/// buffers, and spawns requests for any buffer without a valid cache entry.
	#[cfg(feature = "lsp")]
	pub(super) fn tick_semantic_tokens(&mut self) {
		if self.state.integration.lsp.sync().take_semantic_tokens_refresh() {
			self.state.ui.semantic_token_cache.invalidate_all();
		}

		let viewport_height = self.state.core.viewport.height.unwrap_or(24) as usize;
		let visible_ids = self.base_window().layout.buffer_ids();
		let syntax_styles = self.state.config.config.theme.colors.syntax;

		for &buffer_id in &visible_ids {
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
				continue;
			};
			let scroll_line = buffer.scroll_line;
			let doc_rev = buffer.version();
			let line_lo = scroll_line;
			let line_hi = scroll_line + viewport_height + 2;

			if self.state.ui.semantic_token_cache.get(buffer_id, doc_rev, line_lo, line_hi).is_some() {
				continue;
			}

			if self.state.ui.semantic_token_cache.is_in_flight(buffer_id) {
				continue;
			}

			let generation = self.state.ui.semantic_token_cache.bump_generation(buffer_id);
			let epoch = self.state.ui.semantic_token_cache.epoch();
			self.state.ui.semantic_token_cache.mark_in_flight(buffer_id, generation);
			self.state
				.integration
				.lsp
				.request_semantic_tokens(buffer, generation, epoch, line_lo, line_hi, move |scope| Some(syntax_styles.resolve(scope)));
		}
	}

	/// Ticks document highlights (references under cursor) with debounced requests.
	///
	/// Waits for the cursor to settle for 2 ticks before sending a request,
	/// avoiding excessive requests during rapid cursor movement.
	#[cfg(feature = "lsp")]
	pub(super) fn tick_document_highlights(&mut self) {
		use crate::lsp::document_highlight::DOCUMENT_HIGHLIGHT_SETTLE_TICKS;

		let visible_ids = self.base_window().layout.buffer_ids();

		for &buffer_id in &visible_ids {
			let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
				continue;
			};
			let cursor = buffer.cursor;
			let doc_rev = buffer.version();

			// Check cache hit — already have highlights for this cursor position.
			if self.state.ui.document_highlight_cache.get(buffer_id, doc_rev, cursor).is_some() {
				continue;
			}

			if self.state.ui.document_highlight_cache.is_in_flight(buffer_id) {
				continue;
			}

			// Debounce: wait for cursor to settle.
			if !self
				.state
				.ui
				.document_highlight_cache
				.tick_settle(buffer_id, cursor, doc_rev, DOCUMENT_HIGHLIGHT_SETTLE_TICKS)
			{
				continue;
			}

			let generation = self.state.ui.document_highlight_cache.bump_generation(buffer_id);
			if self.state.integration.lsp.request_document_highlights(buffer, generation, cursor) {
				self.state.ui.document_highlight_cache.mark_in_flight(buffer_id, generation);
			}
		}
	}

	/// Queues an immediate LSP change and returns a receiver for write completion.
	#[cfg(feature = "lsp")]
	pub(super) fn queue_lsp_change_immediate(&mut self, buffer_id: crate::buffer::ViewId) -> Option<oneshot::Receiver<()>> {
		self.maybe_track_lsp_for_buffer(buffer_id, false);

		let buffer = self.state.core.editor.buffers.get_buffer(buffer_id)?;
		let doc_id = buffer.document_id();
		let snapshot = buffer.with_doc(|doc| (doc.content().clone(), doc.version()));
		let sync = self.state.integration.lsp.sync().clone();
		let metrics = self.state.telemetry.metrics.clone();

		self.state
			.integration
			.lsp
			.sync_manager_mut()
			.flush_now(Instant::now(), doc_id, &sync, &metrics, Some(snapshot))
	}

	/// Immediately flush LSP changes for specified buffers.
	#[cfg(feature = "lsp")]
	pub fn flush_lsp_sync_now(&mut self, buffer_ids: &[crate::buffer::ViewId]) -> FlushHandle {
		let mut handles = Vec::new();
		for &buffer_id in buffer_ids {
			if let Some(handle) = self.queue_lsp_change_immediate(buffer_id) {
				handles.push(handle);
			}
		}
		FlushHandle { handles }
	}

	/// Collects a snapshot of current editor statistics.
	pub fn stats_snapshot(&self) -> StatsSnapshot {
		#[cfg(feature = "lsp")]
		let (lsp_pending_docs, lsp_in_flight) = (
			self.state.integration.lsp.sync_manager().pending_count(),
			self.state.integration.lsp.sync_manager().in_flight_count(),
		);
		#[cfg(not(feature = "lsp"))]
		let (lsp_pending_docs, lsp_in_flight) = (0, 0);

		let nu = crate::metrics::NuStats {
			runtime_loaded: self.state.integration.nu.runtime().is_some(),
			script_path: self
				.state
				.integration
				.nu
				.runtime()
				.as_ref()
				.map(|rt| rt.script_path().to_string_lossy().to_string()),
			executor_alive: self.state.integration.nu.executor().is_some(),
			hook_phase: self.state.integration.nu.hook_phase().label(),
			hook_queue_len: self.state.integration.nu.hook_queue_len(),
			hook_in_flight: self
				.state
				.integration
				.nu
				.hook_in_flight()
				.map(|i| (i.token.runtime_epoch, i.token.seq, "on_hook".to_string())),
			runtime_work_queue_len: self.runtime_work_len(),
			hook_dropped_total: self.state.integration.nu.hook_dropped_total(),
			runtime_epoch: self.state.integration.nu.runtime_epoch(),
			hook_eval_seq_next: self.state.integration.nu.hook_eval_seq_next(),
		};

		StatsSnapshot {
			hooks_pending: self.state.integration.work_scheduler.pending_count(),
			hooks_scheduled: self.state.integration.work_scheduler.scheduled_total(),
			hooks_completed: self.state.integration.work_scheduler.completed_total(),
			hooks_completed_tick: self.state.telemetry.metrics.hooks_completed_tick_count(),
			hooks_pending_tick: self.state.telemetry.metrics.hooks_pending_tick_count(),
			lsp_pending_docs,
			lsp_in_flight,
			lsp_full_sync: self.state.telemetry.metrics.full_sync_count(),
			lsp_incremental_sync: self.state.telemetry.metrics.incremental_sync_count(),
			lsp_send_errors: self.state.telemetry.metrics.send_error_count(),
			lsp_coalesced: self.state.telemetry.metrics.coalesced_count(),
			lsp_snapshot_bytes: self.state.telemetry.metrics.snapshot_bytes_count(),
			lsp_full_sync_tick: self.state.telemetry.metrics.full_sync_tick_count(),
			lsp_incremental_sync_tick: self.state.telemetry.metrics.incremental_sync_tick_count(),
			lsp_snapshot_bytes_tick: self.state.telemetry.metrics.snapshot_bytes_tick_count(),
			nu,
		}
	}
}

#[cfg(test)]
mod tests;
