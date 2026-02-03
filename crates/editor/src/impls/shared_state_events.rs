//! Shared state event drain and handlers.
//!
//! Processes inbound [`SharedStateEvent`]s from the broker during the editor
//! tick, applying remote deltas to local buffers and updating sync state.

use xeno_broker_proto::types::RequestPayload;
use xeno_primitives::{SyntaxPolicy, UndoPolicy};

use super::Editor;
use crate::buffer::ApplyPolicy;
use crate::shared_state::SharedStateEvent;

impl Editor {
	/// Drains all pending shared state events from the broker transport.
	///
	/// Executed once per editor tick. Coordinates the synchronization
	/// pipeline:
	/// 1. Drain and apply inbound events (deltas, ownership changes).
	/// 2. Emit full resync requests for diverged documents.
	/// 3. Emit queued edit requests once acknowledgments arrive.
	pub(crate) fn drain_shared_state_events(&mut self) {
		while let Some(event) = self.state.lsp.try_recv_shared_state_in() {
			self.handle_shared_state_event(event);
		}

		for req in self.state.shared_state.drain_resync_requests() {
			let (client_len, client_hash) = self
				.state
				.core
				.buffers
				.any_buffer_for_doc(req.doc_id)
				.and_then(|view_id| self.state.core.buffers.get_buffer(view_id))
				.map(|buffer| {
					buffer.with_doc(|doc| xeno_broker_proto::fingerprint_rope(doc.content()))
				})
				.map(|(len, hash)| (Some(len), Some(hash)))
				.unwrap_or((None, None));

			let payload = RequestPayload::SharedResync {
				uri: req.uri,
				client_hash64: client_hash,
				client_len_chars: client_len,
			};
			let _ = self.state.lsp.shared_state_out_tx().send(payload);
		}

		for payload in self.state.shared_state.drain_pending_edit_requests() {
			let _ = self.state.lsp.shared_state_out_tx().send(payload);
		}
	}

	/// Dispatches a single shared state event to the appropriate handler.
	fn handle_shared_state_event(&mut self, event: SharedStateEvent) {
		match event {
			SharedStateEvent::RemoteDelta {
				uri,
				epoch,
				seq,
				tx,
			} => self.apply_remote_shared_delta(&uri, epoch, seq, &tx),

			SharedStateEvent::OwnerChanged { snapshot }
			| SharedStateEvent::PreferredOwnerChanged { snapshot }
			| SharedStateEvent::FocusAck { snapshot } => {
				let local_session = self.state.lsp.broker_session_id();
				let uri = snapshot.uri.clone();
				self.state
					.shared_state
					.handle_snapshot_update(snapshot, local_session);
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}
			SharedStateEvent::Unlocked { snapshot } => {
				let local_session = self.state.lsp.broker_session_id();
				let uri = snapshot.uri.clone();
				self.state
					.shared_state
					.handle_snapshot_update(snapshot, local_session);
				self.maybe_request_shared_focus(&uri);
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::Opened { snapshot, text } => {
				let local_session = self.state.lsp.broker_session_id();
				let uri = snapshot.uri.clone();
				let opened_text =
					self.state
						.shared_state
						.handle_opened(snapshot, text, local_session);
				if let Some(text) = opened_text {
					self.apply_sync_snapshot(&uri, &text);
				}
				self.maybe_request_shared_focus(&uri);
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::EditAck { uri, epoch, seq } => {
				self.state.shared_state.handle_edit_ack(&uri, epoch, seq);
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::Snapshot {
				uri,
				text,
				snapshot,
			} => {
				let local_session = self.state.lsp.broker_session_id();
				let local_fingerprint = self
					.state
					.shared_state
					.doc_id_for_uri(&uri)
					.and_then(|doc_id| self.state.core.buffers.any_buffer_for_doc(doc_id))
					.and_then(|view_id| self.state.core.buffers.get_buffer(view_id))
					.map(|buffer| {
						buffer.with_doc(|doc| xeno_broker_proto::fingerprint_rope(doc.content()))
					});
				let (local_len, local_hash) = local_fingerprint
					.map(|(len, hash)| (Some(len), Some(hash)))
					.unwrap_or((None, None));

				self.state
					.shared_state
					.handle_snapshot(&uri, snapshot.clone(), local_session);

				if crate::shared_state::should_apply_snapshot_text(
					&text, &snapshot, local_len, local_hash,
				) {
					self.apply_sync_snapshot(&uri, &text);
				}
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::RequestFailed { uri } => {
				self.state.shared_state.handle_request_failed(&uri);
				self.update_readonly_for_shared_state(&uri);
			}

			SharedStateEvent::EditRejected { uri } => {
				self.state.shared_state.mark_needs_resync(&uri);
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::Disconnected => {
				self.handle_shared_state_disconnect();
			}
		}
	}

	/// Disables all shared state and clears readonly overrides for tracked docs
	/// so the editor can resume local-only editing.
	fn handle_shared_state_disconnect(&mut self) {
		let blocked_doc_ids: Vec<_> = self
			.state
			.core
			.buffers
			.buffer_ids()
			.filter_map(|id| {
				let buffer = self.state.core.buffers.get_buffer(id)?;
				let doc_id = buffer.document_id();
				let uri = self.state.shared_state.uri_for_doc_id(doc_id)?;
				self.state
					.shared_state
					.is_edit_blocked(uri)
					.then_some(doc_id)
			})
			.collect();

		for buffer in self.state.core.buffers.buffers_mut() {
			if blocked_doc_ids.contains(&buffer.document_id()) {
				buffer.set_readonly_override(None);
			}
		}

		self.state.shared_state.disable_all();
		self.state.frame.needs_redraw = true;
	}

	/// Validates, converts, and applies a remote delta to the local buffer.
	fn apply_remote_shared_delta(
		&mut self,
		uri: &str,
		epoch: xeno_broker_proto::types::SyncEpoch,
		seq: xeno_broker_proto::types::SyncSeq,
		wire_tx: &xeno_broker_proto::types::WireTx,
	) {
		let Some(doc_id) = self.state.shared_state.handle_remote_delta(uri, epoch, seq) else {
			return;
		};

		let Some(view_id) = self.state.core.buffers.any_buffer_for_doc(doc_id) else {
			return;
		};

		let Some(buffer) = self.state.core.buffers.get_buffer(view_id) else {
			return;
		};

		let tx = buffer.with_doc(|doc| {
			crate::shared_state::convert::wire_to_tx(wire_tx, doc.content().slice(..))
		});

		let policy = ApplyPolicy {
			undo: UndoPolicy::NoUndo,
			syntax: SyntaxPolicy::IncrementalOrDirty,
		};

		let apply = {
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer_mut(view_id)
				.expect("buffer must exist");
			buffer.apply_remote(&tx, policy, &self.state.config.language_loader)
		};

		if !apply.applied {
			return;
		}

		self.state.syntax_manager.note_edit(doc_id);
		self.state.frame.dirty_buffers.insert(view_id);
		self.state.frame.needs_redraw = true;
	}

	/// Replaces document content from a full sync snapshot.
	fn apply_sync_snapshot(&mut self, uri: &str, text: &str) {
		let Some(doc_id) = self.state.shared_state.doc_id_for_uri(uri) else {
			return;
		};
		let Some(view_id) = self.state.core.buffers.any_buffer_for_doc(doc_id) else {
			return;
		};

		if let Some(buffer) = self.state.core.buffers.get_buffer(view_id) {
			buffer.with_doc_mut(|doc| doc.install_sync_snapshot(text));
		}

		let view_ids: Vec<_> = self.state.core.buffers.views_for_doc(doc_id).to_vec();
		for vid in view_ids {
			if let Some(buf) = self.state.core.buffers.get_buffer_mut(vid) {
				buf.ensure_valid_selection();
			}
		}

		self.state.syntax_manager.note_edit(doc_id);
		self.state.frame.dirty_buffers.insert(view_id);
	}

	/// Updates readonly overrides on all views of a shared document.
	fn update_readonly_for_shared_state(&mut self, uri: &str) {
		let Some(doc_id) = self.state.shared_state.doc_id_for_uri(uri) else {
			return;
		};

		let (_, status) = self.state.shared_state.ui_status_for_uri(uri);
		let override_val = match status {
			crate::shared_state::SyncStatus::NeedsResync => Some(true),
			_ => None,
		};

		let view_ids: Vec<_> = self.state.core.buffers.views_for_doc(doc_id).to_vec();
		for vid in view_ids {
			if let Some(buf) = self.state.core.buffers.get_buffer_mut(vid) {
				buf.set_readonly_override(override_val);
			}
		}
	}

	/// Sends a focus claim for a newly opened document if it is currently focused.
	fn maybe_request_shared_focus(&mut self, uri: &str) {
		let Some(doc_id) = self.state.shared_state.doc_id_for_uri(uri) else {
			return;
		};
		let focused_doc = self
			.state
			.core
			.buffers
			.get_buffer(self.focused_view())
			.map(|buffer| buffer.document_id());
		if focused_doc != Some(doc_id) || self.state.shared_state.is_owner(uri) {
			return;
		}
		if let Some(payload) = self.state.shared_state.note_focus(doc_id, true) {
			let _ = self.state.lsp.shared_state_out_tx().send(payload);
		}
	}
}
