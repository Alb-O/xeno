#[cfg(feature = "lsp")]
use std::time::Instant;

#[cfg(feature = "lsp")]
use tokio::sync::oneshot;
#[cfg(feature = "lsp")]
use tracing::warn;

use super::super::Editor;
#[cfg(feature = "lsp")]
use super::state::FlushHandle;
#[cfg(feature = "lsp")]
use crate::lsp::sync_manager::{LSP_MAX_INCREMENTAL_BYTES, LSP_MAX_INCREMENTAL_CHANGES};
use crate::metrics::StatsSnapshot;
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

impl Editor {
	/// Orchestrates background syntax parsing for all buffers and installs results.
	///
	/// This implementation deduplicates parsing by document, ensuring that documents
	/// shared across multiple views are only processed once.
	///
	/// # Hotness and Retention
	///
	/// The workflow calculates the maximum hotness for each document based on its
	/// visibility in any split or floating window. Non-visible documents are
	/// marked as `Cold` to allow the syntax manager to evict their trees when
	/// memory pressures or TTL thresholds are met.
	///
	/// # Polling Set
	///
	/// The set of documents to poll includes:
	/// 1. All currently visible documents.
	/// 2. Documents with active background tasks (to harvest results).
	/// 3. Documents marked dirty by recent edits.
	pub fn ensure_syntax_for_buffers(&mut self) {
		use std::collections::{HashMap, HashSet};

		use crate::syntax_manager::{EnsureSyntaxContext, SyntaxHotness};

		let loader = std::sync::Arc::clone(&self.state.config.language_loader);
		let mut visible_ids = self.state.windows.base_window().layout.views();
		for (_, floating) in self.state.windows.floating_windows() {
			visible_ids.push(floating.buffer);
		}

		let mut doc_hotness = HashMap::new();

		for buffer_id in self.state.core.buffers.buffer_ids() {
			let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
				continue;
			};

			let doc_id = buffer.document_id();
			let hotness = if visible_ids.contains(&buffer_id) {
				SyntaxHotness::Visible
			} else {
				SyntaxHotness::Cold
			};

			doc_hotness
				.entry(doc_id)
				.and_modify(|h| {
					if hotness == SyntaxHotness::Visible {
						*h = SyntaxHotness::Visible;
					}
				})
				.or_insert(hotness);
		}

		let mut workset: HashSet<_> = self.state.syntax_manager.pending_docs().collect();
		workset.extend(self.state.syntax_manager.dirty_docs());
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
			let Some(uri) = xeno_lsp::uri_from_path(&path) else {
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
		let client_ready = sync.registry().any_server_ready();
		let buffers = &self.state.core.buffers;
		let stats = self.state.lsp.sync_manager_mut().tick(
			Instant::now(),
			client_ready,
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
		let buffer = self.state.core.buffers.get_buffer(buffer_id)?;
		let (path, language) = (buffer.path()?, buffer.file_type()?);
		let doc_id = buffer.document_id();

		let (changes, force_full_sync, total_bytes) =
			self.state.lsp.sync_manager_mut().take_immediate(doc_id)?;

		let change_count = changes.len();
		let use_incremental = !force_full_sync
			&& self
				.state
				.lsp
				.incremental_encoding_for_buffer(buffer)
				.is_some()
			&& change_count > 0
			&& change_count <= LSP_MAX_INCREMENTAL_CHANGES
			&& total_bytes <= LSP_MAX_INCREMENTAL_BYTES;

		let content = buffer.with_doc(|doc| doc.content().clone());
		let sync = self.state.lsp.sync().clone();
		let metrics = self.state.metrics.clone();
		let (tx, rx) = oneshot::channel();

		tokio::spawn(async move {
			let mut snapshot: Option<String> = None;
			let mut snapshot_bytes: Option<u64> = None;
			let mut take_snapshot = || {
				if snapshot.is_none() {
					snapshot_bytes = Some(content.len_bytes() as u64);
					snapshot = Some(content.to_string());
				}
				(snapshot.take().unwrap(), snapshot_bytes.unwrap())
			};

			let result = if use_incremental {
				match sync
					.notify_change_incremental_no_content_with_barrier(&path, &language, changes)
					.await
				{
					Ok(ack) => {
						metrics.inc_incremental_sync();
						Ok(ack)
					}
					Err(_) => {
						metrics.inc_send_error();
						let (snapshot, bytes) = take_snapshot();
						metrics.add_snapshot_bytes(bytes);
						sync.notify_change_full_with_barrier_text(&path, &language, snapshot)
							.await
							.inspect(|_| metrics.inc_full_sync())
							.inspect_err(|_| metrics.inc_send_error())
					}
				}
			} else {
				let (snapshot, bytes) = take_snapshot();
				metrics.add_snapshot_bytes(bytes);
				sync.notify_change_full_with_barrier_text(&path, &language, snapshot)
					.await
					.inspect(|_| metrics.inc_full_sync())
					.inspect_err(|_| metrics.inc_send_error())
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

			self.flush_pending_overlay_commit().await;
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
