//! Buffer sync event drain and handlers.
//!
//! Processes inbound [`BufferSyncEvent`]s from the broker during the editor
//! tick, applying remote deltas to local buffers and updating sync state.

use xeno_broker_proto::types::BufferSyncRole;
use xeno_primitives::{SyntaxPolicy, UndoPolicy};

use super::Editor;
use crate::buffer::ApplyPolicy;
use crate::buffer_sync::BufferSyncEvent;

impl Editor {
	/// Drains all pending buffer sync events from the broker transport.
	///
	/// Called once per editor tick after LSP UI events but before dirty buffer
	/// hooks. After processing events, sends [`RequestPayload::BufferSyncResync`]
	/// for any documents that detected epoch mismatches or sequence gaps.
	pub(crate) fn drain_buffer_sync_events(&mut self) {
		while let Some(event) = self.state.lsp.try_recv_buffer_sync_in() {
			self.handle_buffer_sync_event(event);
		}

		for payload in self.state.buffer_sync.drain_resync_requests() {
			let _ = self.state.lsp.buffer_sync_out_tx().send(payload);
		}
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

			BufferSyncEvent::OwnerChanged { uri, epoch, owner } => {
				let local_session = self.state.lsp.broker_session_id();
				self.state
					.buffer_sync
					.handle_owner_changed(&uri, epoch, owner, local_session);

				let new_role = self.state.buffer_sync.role_for_uri(&uri);
				self.update_readonly_for_sync_role(&uri, new_role);
				self.state.frame.needs_redraw = true;
			}

			BufferSyncEvent::Opened {
				uri,
				role,
				epoch,
				seq,
				snapshot,
			} => {
				if let Some(text) = self
					.state
					.buffer_sync
					.handle_opened(&uri, role, epoch, seq, snapshot)
				{
					self.apply_sync_snapshot(&uri, &text);
				}

				self.update_readonly_for_sync_role(&uri, Some(role));
				self.state.frame.needs_redraw = true;
			}

			BufferSyncEvent::DeltaAck { uri, seq } => {
				self.state.buffer_sync.handle_delta_ack(&uri, seq);
			}

			BufferSyncEvent::DeltaRejected { uri } => {
				self.state.buffer_sync.mark_needs_resync(&uri);
			}

			BufferSyncEvent::Snapshot {
				uri,
				text,
				epoch,
				seq,
				owner,
			} => {
				let local_session = self.state.lsp.broker_session_id();
				self.state
					.buffer_sync
					.handle_owner_changed(&uri, epoch, owner, local_session);
				self.state.buffer_sync.handle_delta_ack(&uri, seq);

				self.apply_sync_snapshot(&uri, &text);
				self.state.buffer_sync.clear_needs_resync(&uri);

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
				(self.state.buffer_sync.is_follower(uri)
					|| self.state.buffer_sync.needs_resync(uri))
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
	/// clear the override unless they are awaiting resync, in which case they
	/// remain readonly until a snapshot arrives.
	fn update_readonly_for_sync_role(&mut self, uri: &str, role: Option<BufferSyncRole>) {
		let Some(doc_id) = self.state.buffer_sync.doc_id_for_uri(uri) else {
			return;
		};

		let needs_resync = self.state.buffer_sync.needs_resync(uri);
		let override_val = match role {
			Some(BufferSyncRole::Follower) => Some(true),
			Some(BufferSyncRole::Owner) if needs_resync => Some(true),
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

#[cfg(test)]
mod tests {
	use xeno_broker_proto::types::{BufferSyncRole, SessionId, SyncEpoch, SyncSeq};

	use super::Editor;

	#[test]
	fn test_new_owner_remains_readonly_until_snapshot() {
		let mut editor = Editor::new_scratch();
		let uri = "file:///test.rs";
		let doc_id = editor.buffer().document_id();

		editor.state.buffer_sync.prepare_open(uri, "hello", doc_id);
		editor.state.buffer_sync.handle_opened(
			uri,
			BufferSyncRole::Follower,
			SyncEpoch(1),
			SyncSeq(0),
			None,
		);

		editor.state.buffer_sync.handle_owner_changed(
			uri,
			SyncEpoch(2),
			SessionId(1),
			SessionId(1),
		);
		editor.update_readonly_for_sync_role(uri, Some(BufferSyncRole::Owner));
		assert!(editor.buffer().is_readonly());

		editor.state.buffer_sync.clear_needs_resync(uri);
		editor.update_readonly_for_sync_role(uri, Some(BufferSyncRole::Owner));
		assert!(!editor.buffer().is_readonly());
	}
}
