#[cfg(feature = "lsp")]
use std::time::Instant;

#[cfg(feature = "lsp")]
use tokio::sync::oneshot;

use super::super::Editor;
#[cfg(feature = "lsp")]
use super::state::FlushHandle;
use crate::metrics::StatsSnapshot;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

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

		use crate::syntax_manager::{EnsureSyntaxContext, SyntaxHotness};

		let loader = std::sync::Arc::clone(&self.state.config.language_loader);
		let visible_ids: HashSet<_> = self
			.state
			.windows
			.base_window()
			.layout
			.views()
			.into_iter()
			.chain(self.state.windows.floating_windows().map(|(_, f)| f.buffer))
			.collect();

		let mut doc_hotness = HashMap::new();
		let mut doc_viewports = HashMap::new();

		for view in self.state.windows.base_window().layout.views() {
			if let Some(buffer) = self.state.core.buffers.get_buffer(view) {
				let doc_id = buffer.document_id();
				let viewport = buffer.with_doc(|doc| {
					let content = doc.content();
					let total_lines = content.len_lines();
					let start_line = buffer.scroll_line.min(total_lines);
					let start_byte = if start_line < total_lines {
						content.line_to_byte(start_line) as u32
					} else {
						content.len_bytes() as u32
					};
					let height = self.view_area(view).height as usize;
					let end_line = start_line.saturating_add(height).min(total_lines);
					let end_byte = if end_line < total_lines {
						content.line_to_byte(end_line) as u32
					} else {
						content.len_bytes() as u32
					};
					start_byte..end_byte
				});
				doc_viewports
					.entry(doc_id)
					.and_modify(|v: &mut std::ops::Range<u32>| {
						v.start = v.start.min(viewport.start);
						v.end = v.end.max(viewport.end);
					})
					.or_insert(viewport);
			}
		}

		for buffer in self.state.core.buffers.buffers() {
			let doc_id = buffer.document_id();
			let hotness = if visible_ids.contains(&buffer.id) {
				self.state.core.warm_docs.touch(doc_id);
				SyntaxHotness::Visible
			} else if self.state.core.warm_docs.contains(doc_id) {
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

		let mut workset: HashSet<_> = self
			.state
			.syntax_manager
			.pending_docs()
			.chain(self.state.syntax_manager.dirty_docs())
			.collect();

		for (&doc_id, &hotness) in &doc_hotness {
			if hotness == SyntaxHotness::Visible {
				workset.insert(doc_id);
			}
		}

		for doc_id in workset {
			let Some(buffer_id) = self.state.core.buffers.any_buffer_for_doc(doc_id) else {
				continue;
			};
			let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
				continue;
			};

			let (version, lang_id, content) =
				buffer.with_doc(|doc| (doc.version(), doc.language_id(), doc.content().clone()));

			let hotness = doc_hotness
				.get(&doc_id)
				.copied()
				.unwrap_or(SyntaxHotness::Warm);

			let viewport = doc_viewports.get(&doc_id).cloned();

			let outcome = self
				.state
				.syntax_manager
				.ensure_syntax(EnsureSyntaxContext {
					doc_id,
					doc_version: version,
					language_id: lang_id,
					content: &content,
					hotness,
					loader: &loader,
					viewport,
				});

			if outcome.updated {
				self.state.effects.request_redraw();
			}
		}
	}

	/// Queues full LSP syncs for documents flagged by the LSP state manager.
	#[cfg(feature = "lsp")]
	pub(super) fn queue_lsp_resyncs_from_documents(&mut self) {
		use std::collections::HashSet;

		let uris = self.state.lsp.documents().take_force_full_sync_uris();
		if uris.is_empty() {
			return;
		}

		let uri_set: HashSet<&str> = uris.iter().map(|u| u.as_str()).collect();
		for buffer in self.state.core.buffers.buffers() {
			let Some(path) = buffer.path() else {
				continue;
			};
			let abs_path = crate::paths::fast_abs(path);
			let Some(uri) = xeno_lsp::uri_from_path(&abs_path) else {
				continue;
			};
			if !uri_set.contains(uri.as_str()) {
				continue;
			}

			self.state
				.lsp
				.sync_manager_mut()
				.escalate_full(buffer.document_id());
		}
	}

	/// Ticks the LSP sync manager, flushing due documents.
	#[cfg(feature = "lsp")]
	pub(super) fn tick_lsp_sync(&mut self) {
		let sync = self.state.lsp.sync().clone();
		let buffers = &self.state.core.buffers;
		let stats = self.state.lsp.sync_manager_mut().tick(
			Instant::now(),
			&sync,
			&self.state.metrics,
			|doc_id| {
				let view_id = buffers.any_buffer_for_doc(doc_id)?;
				let buffer = buffers.get_buffer(view_id)?;
				Some(buffer.with_doc(|doc| (doc.content().clone(), doc.version())))
			},
		);
		self.state.metrics.record_lsp_tick(
			stats.full_syncs,
			stats.incremental_syncs,
			stats.snapshot_bytes,
		);
	}

	/// Queues an immediate LSP change and returns a receiver for write completion.
	#[cfg(feature = "lsp")]
	pub(super) fn queue_lsp_change_immediate(
		&mut self,
		buffer_id: crate::buffer::ViewId,
	) -> Option<oneshot::Receiver<()>> {
		self.maybe_track_lsp_for_buffer(buffer_id, false);

		let buffer = self.state.core.buffers.get_buffer(buffer_id)?;
		let doc_id = buffer.document_id();
		let snapshot = buffer.with_doc(|doc| (doc.content().clone(), doc.version()));
		let sync = self.state.lsp.sync().clone();
		let metrics = self.state.metrics.clone();

		self.state.lsp.sync_manager_mut().flush_now(
			Instant::now(),
			doc_id,
			&sync,
			&metrics,
			Some(snapshot),
		)
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

	/// Drains and executes all queued commands.
	pub async fn drain_command_queue(&mut self) -> bool {
		let commands: Vec<_> = self.state.core.workspace.command_queue.drain().collect();
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
								xeno_registry::notifications::keys::unknown_command(cmd.name),
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
			self.state.lsp.sync_manager().pending_count(),
			self.state.lsp.sync_manager().in_flight_count(),
		);
		#[cfg(not(feature = "lsp"))]
		let (lsp_pending_docs, lsp_in_flight) = (0, 0);

		StatsSnapshot {
			hooks_pending: self.state.hook_runtime.pending_count(),
			hooks_scheduled: self.state.hook_runtime.scheduled_total(),
			hooks_completed: self.state.hook_runtime.completed_total(),
			hooks_completed_tick: self.state.metrics.hooks_completed_tick_count(),
			hooks_pending_tick: self.state.metrics.hooks_pending_tick_count(),
			lsp_pending_docs,
			lsp_in_flight,
			lsp_full_sync: self.state.metrics.full_sync_count(),
			lsp_incremental_sync: self.state.metrics.incremental_sync_count(),
			lsp_send_errors: self.state.metrics.send_error_count(),
			lsp_coalesced: self.state.metrics.coalesced_count(),
			lsp_snapshot_bytes: self.state.metrics.snapshot_bytes_count(),
			lsp_full_sync_tick: self.state.metrics.full_sync_tick_count(),
			lsp_incremental_sync_tick: self.state.metrics.incremental_sync_tick_count(),
			lsp_snapshot_bytes_tick: self.state.metrics.snapshot_bytes_tick_count(),
		}
	}
}

#[cfg(test)]
mod tests;
