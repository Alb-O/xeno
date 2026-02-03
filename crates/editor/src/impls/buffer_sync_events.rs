//! Buffer sync event drain and handlers.
//!
//! Processes inbound [`BufferSyncEvent`]s from the broker during the editor
//! tick, applying remote deltas to local buffers and updating sync state.

use xeno_broker_proto::types::{BufferSyncRole, RequestPayload};
use xeno_primitives::{SyntaxPolicy, UndoPolicy};

use super::Editor;
use crate::buffer::ApplyPolicy;
use crate::buffer_sync::BufferSyncEvent;

impl Editor {
	/// Drains all pending buffer sync events from the broker transport.
	///
	/// Executed once per editor tick. Coordinates the 4-phase synchronization
	/// pipeline:
	/// 1. Drain and apply inbound events (deltas, ownership changes).
	/// 2. Emit ownership alignment confirmations for new local owners.
	/// 3. Emit full resync requests for diverged documents.
	/// 4. Replay edits deferred during ownership transitions.
	pub(crate) fn drain_buffer_sync_events(&mut self) {
		while let Some(event) = self.state.lsp.try_recv_buffer_sync_in() {
			self.handle_buffer_sync_event(event);
		}

		let needs = self.state.buffer_sync.drain_owner_confirm_requests();
		for need in needs {
			if let Some((len_chars, hash64)) = self.compute_doc_fingerprint(need.doc_id) {
				let payload = RequestPayload::BufferSyncOwnerConfirm {
					uri: need.uri,
					epoch: need.epoch,
					len_chars,
					hash64,
					allow_mismatch: need.allow_mismatch,
				};
				let _ = self.state.lsp.buffer_sync_out_tx().send(payload);
			} else {
				self.state.buffer_sync.handle_request_failed(&need.uri);
			}
		}

		for payload in self.state.buffer_sync.drain_resync_requests() {
			let _ = self.state.lsp.buffer_sync_out_tx().send(payload);
		}

		for payload in self.state.buffer_sync.drain_pending_delta_requests() {
			let _ = self.state.lsp.buffer_sync_out_tx().send(payload);
		}

		let ready = self.state.buffer_sync.drain_replay_edits();
		for replay in ready {
			self.apply_edit(
				self.state
					.core
					.buffers
					.any_buffer_for_doc(replay.doc_id)
					.unwrap_or_else(|| self.focused_view()),
				&replay.tx,
				replay.selection,
				replay.undo,
				replay.origin,
			);
		}
	}

	fn compute_doc_fingerprint(&self, doc_id: crate::buffer::DocumentId) -> Option<(u64, u64)> {
		let view_id = self.state.core.buffers.any_buffer_for_doc(doc_id)?;
		let buffer = self.state.core.buffers.get_buffer(view_id)?;
		buffer.with_doc(|doc| Some(xeno_broker_proto::fingerprint_rope(doc.content())))
	}

	/// Dispatches a single buffer sync event to the appropriate handler.
	fn handle_buffer_sync_event(&mut self, event: BufferSyncEvent) {
		match event {
			BufferSyncEvent::RemoteDelta {
				uri,
				epoch,
				seq,
				tx,
			} => self.apply_remote_sync_delta(&uri, epoch, seq, &tx),

			BufferSyncEvent::OwnerChanged { snapshot } => {
				let local_session = self.state.lsp.broker_session_id();
				let uri = snapshot.uri.clone();
				self.state
					.buffer_sync
					.handle_owner_changed(snapshot, local_session);

				let new_role = self.state.buffer_sync.role_for_uri(&uri);
				self.update_readonly_for_sync_role(&uri, new_role);
				self.state.frame.needs_redraw = true;
			}
			BufferSyncEvent::Unlocked { snapshot } => {
				let local_session = self.state.lsp.broker_session_id();
				let uri = snapshot.uri.clone();
				self.state
					.buffer_sync
					.handle_unlocked(snapshot, local_session);
				let new_role = self.state.buffer_sync.role_for_uri(&uri);
				self.update_readonly_for_sync_role(&uri, new_role);
				self.state.frame.needs_redraw = true;
			}

			BufferSyncEvent::OwnershipResult {
				uri: _,
				status,
				snapshot,
			} => {
				use xeno_registry::notifications::keys;
				let local_session = self.state.lsp.broker_session_id();
				let canonical_uri = snapshot.uri.clone();
				self.state
					.buffer_sync
					.handle_ownership_result(status, snapshot, local_session);
				if status == xeno_broker_proto::types::BufferSyncOwnershipStatus::Denied {
					self.notify(keys::SYNC_OWNERSHIP_DENIED);
				}
				let new_role = self.state.buffer_sync.role_for_uri(&canonical_uri);
				self.update_readonly_for_sync_role(&canonical_uri, new_role);
				self.state.frame.needs_redraw = true;
			}

			BufferSyncEvent::OwnerConfirmResult {
				uri: _,
				status,
				snapshot,
				text,
			} => {
				use xeno_broker_proto::types::BufferSyncOwnerConfirmStatus;
				let local_session = self.state.lsp.broker_session_id();
				let canonical_uri = snapshot.uri.clone();
				match status {
					BufferSyncOwnerConfirmStatus::Confirmed => {
						self.state
							.buffer_sync
							.handle_owner_confirm_result(status, snapshot, local_session);
					}
					BufferSyncOwnerConfirmStatus::NeedSnapshot => {
						if let Some(text) = text {
							self.apply_sync_snapshot(&canonical_uri, &text);
							self.state.buffer_sync.handle_snapshot(
								&canonical_uri,
								text,
								snapshot,
								local_session,
							);
						}
					}
				}
				let new_role = self.state.buffer_sync.role_for_uri(&canonical_uri);
				self.update_readonly_for_sync_role(&canonical_uri, new_role);
				self.state.frame.needs_redraw = true;
			}

			BufferSyncEvent::Opened { snapshot, text } => {
				let local_session = self.state.lsp.broker_session_id();
				let uri = snapshot.uri.clone();
				if let Some(text) = self
					.state
					.buffer_sync
					.handle_opened(snapshot, text, local_session)
				{
					self.apply_sync_snapshot(&uri, &text);
				}

				let new_role = self.state.buffer_sync.role_for_uri(&uri);
				self.update_readonly_for_sync_role(&uri, new_role);
				self.state.frame.needs_redraw = true;
			}

			BufferSyncEvent::DeltaAck { uri, seq } => {
				self.state.buffer_sync.handle_delta_ack(&uri, seq);
			}

			BufferSyncEvent::DeltaRejected { uri } => {
				self.state.buffer_sync.mark_needs_resync(&uri);
			}

			BufferSyncEvent::RequestFailed { uri } => {
				self.state.buffer_sync.handle_request_failed(&uri);
				let new_role = self.state.buffer_sync.role_for_uri(&uri);
				self.update_readonly_for_sync_role(&uri, new_role);
			}

			BufferSyncEvent::Snapshot {
				uri,
				text,
				snapshot,
			} => {
				let local_session = self.state.lsp.broker_session_id();
				self.state.buffer_sync.handle_snapshot(
					&uri,
					text.clone(),
					snapshot,
					local_session,
				);

				self.apply_sync_snapshot(&uri, &text);

				let new_role = self.state.buffer_sync.role_for_uri(&uri);
				self.update_readonly_for_sync_role(&uri, new_role);
				self.state.frame.needs_redraw = true;
			}

			BufferSyncEvent::Disconnected => {
				self.handle_buffer_sync_disconnect();
			}
		}
	}

	/// Disables all sync state and clears readonly overrides for buffer-sync
	/// readonly documents so the editor can resume local-only editing.
	///
	/// Only clears overrides for documents that were tracked as followers or
	/// awaiting resync, preserving overrides set by other subsystems.
	fn handle_buffer_sync_disconnect(&mut self) {
		let blocked_doc_ids: Vec<_> = self
			.state
			.core
			.buffers
			.buffer_ids()
			.filter_map(|id| {
				let buffer = self.state.core.buffers.get_buffer(id)?;
				let doc_id = buffer.document_id();
				let uri = self.state.buffer_sync.uri_for_doc_id(doc_id)?;
				self.state
					.buffer_sync
					.is_edit_blocked(uri)
					.then_some(doc_id)
			})
			.collect();

		for buffer in self.state.core.buffers.buffers_mut() {
			if blocked_doc_ids.contains(&buffer.document_id()) {
				buffer.set_readonly_override(None);
			}
		}

		self.state.buffer_sync.disable_all();
		self.state.frame.needs_redraw = true;
	}

	/// Validates, converts, and applies a remote sync delta to the local buffer.
	///
	/// Converts the wire transaction against the actual document rope and applies
	/// via [`Buffer::apply_remote`] which bypasses the follower readonly override.
	/// Maps selections for all views of the affected document afterward.
	///
	/// [`Buffer::apply_remote`]: crate::buffer::Buffer::apply_remote
	fn apply_remote_sync_delta(
		&mut self,
		uri: &str,
		epoch: xeno_broker_proto::types::SyncEpoch,
		seq: xeno_broker_proto::types::SyncSeq,
		wire_tx: &xeno_broker_proto::types::WireTx,
	) {
		let Some(doc_id) = self.state.buffer_sync.handle_remote_delta(uri, epoch, seq) else {
			return;
		};

		let Some(view_id) = self.state.core.buffers.any_buffer_for_doc(doc_id) else {
			return;
		};

		let Some(tx) = self.state.core.buffers.get_buffer(view_id).map(|b| {
			b.with_doc(|doc| {
				crate::buffer_sync::convert::wire_to_tx(wire_tx, doc.content().slice(..))
			})
		}) else {
			return;
		};

		let policy = ApplyPolicy {
			undo: UndoPolicy::NoUndo,
			syntax: SyntaxPolicy::IncrementalOrDirty,
		};
		let applied = self
			.state
			.core
			.buffers
			.get_buffer(view_id)
			.is_some_and(|b| {
				b.apply_remote(&tx, policy, &self.state.config.language_loader)
					.applied
			});

		if !applied {
			return;
		}

		let view_ids: Vec<_> = self.state.core.buffers.views_for_doc(doc_id).to_vec();
		for vid in view_ids {
			if let Some(buf) = self.state.core.buffers.get_buffer_mut(vid) {
				buf.map_selection_through(&tx);
				buf.ensure_valid_selection();
			}
		}

		self.state.syntax_manager.note_edit(doc_id);
		self.state.frame.dirty_buffers.insert(view_id);
		self.state.frame.needs_redraw = true;
	}

	/// Replaces document content from a full sync snapshot.
	///
	/// Uses [`Document::install_sync_snapshot`] which preserves monotonic
	/// version numbering and marks the document as modified.
	fn apply_sync_snapshot(&mut self, uri: &str, text: &str) {
		let Some(doc_id) = self.state.buffer_sync.doc_id_for_uri(uri) else {
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

	/// Updates readonly overrides on all views of a synced document.
	///
	/// Follower buffers get `readonly_override = Some(true)`. Owner buffers
	/// clear the override unless they are blocked on confirmation or resync.
	fn update_readonly_for_sync_role(&mut self, uri: &str, role: Option<BufferSyncRole>) {
		let Some(doc_id) = self.state.buffer_sync.doc_id_for_uri(uri) else {
			return;
		};

		let is_blocked = self.state.buffer_sync.is_edit_blocked(uri);
		let unlocked = self.state.buffer_sync.is_unlocked(uri);
		let override_val = match role {
			Some(BufferSyncRole::Follower) if !unlocked => Some(true),
			Some(BufferSyncRole::Owner) if is_blocked => Some(true),
			_ => None,
		};

		let view_ids: Vec<_> = self.state.core.buffers.views_for_doc(doc_id).to_vec();
		for vid in view_ids {
			if let Some(buf) = self.state.core.buffers.get_buffer_mut(vid) {
				buf.set_readonly_override(override_val);
			}
		}
	}
}
