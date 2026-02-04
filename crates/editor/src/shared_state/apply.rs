//! Edit, undo, and redo mutation preparation and acknowledgment.

use xeno_broker_proto::types::{RequestPayload, SharedApplyKind, SyncEpoch, SyncSeq, WireTx};
use xeno_primitives::Transaction;

use super::convert;
use super::manager::SharedStateManager;
use super::types::{InFlightEdit, SharedStateRole};

impl SharedStateManager {
	/// Prepares an authoritative mutation request.
	///
	/// Pipelining: If a request is already in-flight, edits are queued in
	/// `pending_deltas` to be drained after acknowledgment.
	fn prepare_apply(
		&mut self,
		uri: &str,
		kind: SharedApplyKind,
		tx: Option<WireTx>,
	) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		if entry.role != SharedStateRole::Owner || entry.needs_resync {
			return None;
		}

		let group_id = entry.current_undo_group;

		if entry.in_flight.is_some() {
			if kind == SharedApplyKind::Edit
				&& let Some(tx) = tx
			{
				entry.pending_deltas.push_back((tx, group_id));
			} else {
				entry.pending_history.push_back(kind);
			}
			return None;
		}

		entry.in_flight = Some(InFlightEdit {
			epoch: entry.epoch,
			base_seq: entry.seq,
		});

		Some(RequestPayload::SharedApply {
			uri: uri.to_string(),
			kind,
			epoch: entry.epoch,
			base_seq: entry.seq,
			base_hash64: entry.auth_hash64,
			base_len_chars: entry.auth_len_chars,
			tx,
			undo_group: group_id,
		})
	}

	/// Prepares a [`SharedApplyKind::Edit`] request.
	pub fn prepare_edit(
		&mut self,
		uri: &str,
		tx: &Transaction,
		new_group: bool,
	) -> Option<RequestPayload> {
		let entry = self.docs.get_mut(uri)?;
		if new_group {
			entry.current_undo_group = entry.current_undo_group.wrapping_add(1).max(1);
		}

		let wire = convert::tx_to_wire(tx);
		self.prepare_apply(uri, SharedApplyKind::Edit, Some(wire))
	}

	/// Prepares a [`SharedApplyKind::Undo`] request.
	pub fn prepare_undo(&mut self, uri: &str) -> Option<RequestPayload> {
		self.prepare_apply(uri, SharedApplyKind::Undo, None)
	}

	/// Prepares a [`SharedApplyKind::Redo`] request.
	pub fn prepare_redo(&mut self, uri: &str) -> Option<RequestPayload> {
		self.prepare_apply(uri, SharedApplyKind::Redo, None)
	}

	/// Handles an application acknowledgment from the broker.
	///
	/// Clears the in-flight guard and advances the authoritative fingerprint.
	/// Returns the [`WireTx`] if the broker provided a result the client must apply.
	pub fn handle_apply_ack(
		&mut self,
		uri: &str,
		_kind: SharedApplyKind,
		epoch: SyncEpoch,
		seq: SyncSeq,
		applied_tx: Option<WireTx>,
		hash64: u64,
		len_chars: u64,
		_history_from: Option<u64>,
		_history_to: Option<u64>,
		_history_group: Option<u64>,
	) -> Option<WireTx> {
		let entry = self.docs.get_mut(uri)?;
		let in_flight = entry.in_flight?;

		let expected = in_flight.base_seq.0.wrapping_add(1);
		if epoch == in_flight.epoch && seq.0 == expected {
			entry.seq = seq;
			entry.auth_hash64 = hash64;
			entry.auth_len_chars = len_chars;
			entry.in_flight = None;
			return applied_tx;
		}

		tracing::warn!(
			?uri,
			"stale or mismatched SharedApplyAck ignored: got={epoch:?}/{seq:?}, expected={:?}/{}",
			in_flight.epoch,
			expected
		);
		entry.needs_resync = true;
		entry.resync_requested = false;
		entry.pending_deltas.clear();
		entry.pending_history.clear();
		entry.in_flight = None;
		entry.pending_align = None;
		None
	}

	/// Collects queued edit requests once the in-flight delta is acknowledged.
	pub fn drain_pending_edit_requests(&mut self) -> Vec<RequestPayload> {
		let mut out = Vec::new();

		for (uri, entry) in &mut self.docs {
			if entry.role == SharedStateRole::Owner
				&& !entry.needs_resync
				&& entry.in_flight.is_none()
			{
				if let Some((tx, gid)) = entry.pending_deltas.pop_front() {
					entry.in_flight = Some(InFlightEdit {
						epoch: entry.epoch,
						base_seq: entry.seq,
					});

					out.push(RequestPayload::SharedApply {
						uri: uri.clone(),
						kind: SharedApplyKind::Edit,
						epoch: entry.epoch,
						base_seq: entry.seq,
						base_hash64: entry.auth_hash64,
						base_len_chars: entry.auth_len_chars,
						tx: Some(tx),
						undo_group: gid,
					});
					continue;
				}

				if let Some(kind) = entry.pending_history.pop_front() {
					entry.in_flight = Some(InFlightEdit {
						epoch: entry.epoch,
						base_seq: entry.seq,
					});
					out.push(RequestPayload::SharedApply {
						uri: uri.clone(),
						kind,
						epoch: entry.epoch,
						base_seq: entry.seq,
						base_hash64: entry.auth_hash64,
						base_len_chars: entry.auth_len_chars,
						tx: None,
						undo_group: entry.current_undo_group,
					});
				}
			}
		}

		out
	}
}
