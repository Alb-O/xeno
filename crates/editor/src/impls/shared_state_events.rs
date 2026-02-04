//! Shared state event drain and handlers.
//!
//! Processes inbound [`SharedStateEvent`]s from the broker during the editor
//! tick, applying remote deltas to local buffers and updating sync state.

use xeno_primitives::{SyntaxPolicy, UndoPolicy};
use xeno_registry::notifications::keys;

use super::Editor;
use super::undo_host::EditorUndoHost;
use crate::buffer::ApplyPolicy;
use crate::shared_state::SharedStateEvent;

impl Editor {
	/// Drains all pending shared state events from the broker transport.
	///
	/// This method is executed once per editor tick to synchronize local state
	/// with the broker's authoritative truth. It performs three primary roles:
	/// 1. Drains and applies inbound async events (deltas, ownership changes).
	/// 2. Emits full resync requests for documents that have diverged.
	/// 3. Emits queued edit requests once acknowledgments arrive.
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

			if let Some(payload) =
				self.state
					.shared_state
					.prepare_resync(&req.uri, client_hash, client_len)
			{
				let _ = self.state.lsp.shared_state_out_tx().send(payload);
			}
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
				kind: _,
				tx,
				hash64,
				len_chars,
			} => self.apply_remote_shared_delta(&uri, epoch, seq, &tx, hash64, len_chars),

			SharedStateEvent::OwnerChanged { snapshot }
			| SharedStateEvent::PreferredOwnerChanged { snapshot } => {
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

			SharedStateEvent::ApplyAck {
				uri,
				kind,
				epoch,
				seq,
				applied_tx,
				hash64,
				len_chars,
			} => {
				if let Some(tx) = self
					.state
					.shared_state
					.handle_apply_ack(&uri, kind, epoch, seq, applied_tx, hash64, len_chars)
				{
					self.apply_local_shared_delta_from_ack(&uri, &tx);
				}

				for payload in self.state.shared_state.drain_pending_edit_requests() {
					let _ = self.state.lsp.shared_state_out_tx().send(payload);
				}

				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::Snapshot {
				uri,
				nonce,
				text,
				snapshot,
			} => {
				let local_session = self.state.lsp.broker_session_id();
				let repair_text = self.state.shared_state.handle_snapshot_response(
					&uri,
					snapshot,
					nonce,
					text,
					local_session,
				);
				if let Some(text) = repair_text {
					self.apply_sync_snapshot(&uri, &text);
				}
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::FocusAck {
				nonce,
				snapshot,
				repair_text,
			} => {
				let local_session = self.state.lsp.broker_session_id();
				let uri = snapshot.uri.clone();
				let repair = self.state.shared_state.handle_focus_ack(
					snapshot,
					nonce,
					repair_text,
					local_session,
				);
				if let Some(text) = repair {
					self.apply_sync_snapshot(&uri, &text);
				}
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::RequestFailed { uri } => {
				self.state.shared_state.handle_request_failed(&uri);
				self.update_readonly_for_shared_state(&uri);

				let focused_view = self.focused_view();
				let core = &mut self.state.core;
				let mut host = EditorUndoHost {
					buffers: &mut core.buffers,
					focused_view,
					config: &self.state.config,
					frame: &mut self.state.frame,
					notifications: &mut self.state.notifications,
					syntax_manager: &mut self.state.syntax_manager,
					#[cfg(feature = "lsp")]
					lsp: &mut self.state.lsp,
					#[cfg(feature = "lsp")]
					shared_state: &mut self.state.shared_state,
				};
				core.undo_manager.cancel_pending_history_any(&mut host);
			}

			SharedStateEvent::EditRejected { uri } => {
				self.state.shared_state.mark_needs_resync(&uri);
				self.update_readonly_for_shared_state(&uri);
				self.state.frame.needs_redraw = true;

				let focused_view = self.focused_view();
				let core = &mut self.state.core;
				let mut host = EditorUndoHost {
					buffers: &mut core.buffers,
					focused_view,
					config: &self.state.config,
					frame: &mut self.state.frame,
					notifications: &mut self.state.notifications,
					syntax_manager: &mut self.state.syntax_manager,
					#[cfg(feature = "lsp")]
					lsp: &mut self.state.lsp,
					#[cfg(feature = "lsp")]
					shared_state: &mut self.state.shared_state,
				};
				core.undo_manager.cancel_pending_history_any(&mut host);
			}

			SharedStateEvent::NothingToUndo { uri } => {
				self.update_readonly_for_shared_state(&uri);

				let focused_view = self.focused_view();
				let core = &mut self.state.core;
				let mut host = EditorUndoHost {
					buffers: &mut core.buffers,
					focused_view,
					config: &self.state.config,
					frame: &mut self.state.frame,
					notifications: &mut self.state.notifications,
					syntax_manager: &mut self.state.syntax_manager,
					#[cfg(feature = "lsp")]
					lsp: &mut self.state.lsp,
					#[cfg(feature = "lsp")]
					shared_state: &mut self.state.shared_state,
				};
				core.undo_manager
					.cancel_pending_history(&mut host, crate::types::HistoryKind::Undo);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::NothingToRedo { uri } => {
				self.update_readonly_for_shared_state(&uri);

				let focused_view = self.focused_view();
				let core = &mut self.state.core;
				let mut host = EditorUndoHost {
					buffers: &mut core.buffers,
					focused_view,
					config: &self.state.config,
					frame: &mut self.state.frame,
					notifications: &mut self.state.notifications,
					syntax_manager: &mut self.state.syntax_manager,
					#[cfg(feature = "lsp")]
					lsp: &mut self.state.lsp,
					#[cfg(feature = "lsp")]
					shared_state: &mut self.state.shared_state,
				};
				core.undo_manager
					.cancel_pending_history(&mut host, crate::types::HistoryKind::Redo);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::HistoryUnavailable { uri } => {
				self.update_readonly_for_shared_state(&uri);
				self.notify(keys::SYNC_HISTORY_UNAVAILABLE);

				let focused_view = self.focused_view();
				let core = &mut self.state.core;
				let mut host = EditorUndoHost {
					buffers: &mut core.buffers,
					focused_view,
					config: &self.state.config,
					frame: &mut self.state.frame,
					notifications: &mut self.state.notifications,
					syntax_manager: &mut self.state.syntax_manager,
					#[cfg(feature = "lsp")]
					lsp: &mut self.state.lsp,
					#[cfg(feature = "lsp")]
					shared_state: &mut self.state.shared_state,
				};
				core.undo_manager.cancel_pending_history_any(&mut host);
				self.state.frame.needs_redraw = true;
			}

			SharedStateEvent::Disconnected => {
				self.handle_shared_state_disconnect();
			}
		}
	}

	/// Disables all shared state and clears readonly overrides for tracked docs.
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
		hash64: u64,
		len_chars: u64,
	) {
		let Some(doc_id) = self
			.state
			.shared_state
			.handle_remote_delta(uri, epoch, seq, hash64, len_chars)
		else {
			return;
		};

		self.apply_shared_delta_to_buffer(doc_id, wire_tx);
		self.update_readonly_for_shared_state(uri);
	}

	/// Applies a shared state delta received in an ApplyAck (intended for owners).
	fn apply_local_shared_delta_from_ack(
		&mut self,
		uri: &str,
		wire_tx: &xeno_broker_proto::types::WireTx,
	) {
		let Some(doc_id) = self.state.shared_state.doc_id_for_uri(uri) else {
			return;
		};
		self.apply_shared_delta_to_buffer(doc_id, wire_tx);
		self.update_readonly_for_shared_state(uri);
	}

	/// Internal helper to convert and apply a delta to all views of a document.
	fn apply_shared_delta_to_buffer(
		&mut self,
		doc_id: crate::buffer::DocumentId,
		wire_tx: &xeno_broker_proto::types::WireTx,
	) {
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

		let focused_view = self.focused_view();
		let core = &mut self.state.core;
		let mut host = EditorUndoHost {
			buffers: &mut core.buffers,
			focused_view,
			config: &self.state.config,
			frame: &mut self.state.frame,
			notifications: &mut self.state.notifications,
			syntax_manager: &mut self.state.syntax_manager,
			#[cfg(feature = "lsp")]
			lsp: &mut self.state.lsp,
			#[cfg(feature = "lsp")]
			shared_state: &mut self.state.shared_state,
		};
		core.undo_manager
			.note_remote_history_delta(&mut host, doc_id);
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
	pub(crate) fn update_readonly_for_shared_state(&mut self, uri: &str) {
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
		let focused_view = self.focused_view();
		let focused_doc = self
			.state
			.core
			.buffers
			.get_buffer(focused_view)
			.map(|buffer| buffer.document_id());

		if focused_doc != Some(doc_id) || self.state.shared_state.is_owner(uri) {
			return;
		}

		let fingerprint = self
			.state
			.core
			.buffers
			.get_buffer(focused_view)
			.map(|b| b.with_doc(|doc| xeno_broker_proto::fingerprint_rope(doc.content())));
		let (len, hash) = fingerprint
			.map(|(l, h)| (Some(l), Some(h)))
			.unwrap_or((None, None));

		if let Some(payload) = self
			.state
			.shared_state
			.prepare_focus(doc_id, true, hash, len)
		{
			let _ = self.state.lsp.shared_state_out_tx().send(payload);
		}
	}
}
