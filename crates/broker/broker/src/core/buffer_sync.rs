//! Buffer synchronization protocol handlers.
//!
//! Implements the single-writer buffer sync protocol with epoch/sequence ordering.

use std::collections::HashMap;
use std::sync::Arc;

use ropey::Rope;
use xeno_broker_proto::types::{
	BufferSyncRole, ErrorCode, Event, IpcFrame, ResponsePayload, SessionId, SyncEpoch, SyncSeq,
	WireTx,
};

use super::BrokerCore;

impl BrokerCore {
	/// Handle a `BufferSyncOpen` request.
	///
	/// First opener becomes the owner (epoch=1, seq=0). Subsequent openers
	/// become followers and receive the current snapshot.
	pub fn on_buffer_sync_open(
		&self,
		session_id: SessionId,
		uri_in: &str,
		text: &str,
		_version_hint: Option<u32>,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = Self::normalize_uri(uri_in)?;
		let (role, epoch, seq, snapshot_rope) = {
			let mut sync = self.sync.lock().unwrap();

			match sync.sync_docs.get_mut(&uri) {
				None => {
					let mut doc = super::SyncDocState {
						owner: session_id,
						open_refcounts: HashMap::new(),
						participants: Vec::new(),
						epoch: SyncEpoch(1),
						seq: SyncSeq(0),
						rope: Rope::from(text),
						owner_needs_resync: false,
					};
					doc.add_open(session_id);
					sync.sync_docs.insert(uri.clone(), doc);
					(BufferSyncRole::Owner, SyncEpoch(1), SyncSeq(0), None)
				}
				Some(doc) => {
					doc.add_open(session_id);
					(
						BufferSyncRole::Follower,
						doc.epoch,
						doc.seq,
						Some(doc.rope.clone()),
					)
				}
			}
		};

		let response = ResponsePayload::BufferSyncOpened {
			role,
			epoch,
			seq,
			snapshot: snapshot_rope.map(|rope| rope.to_string()),
		};

		if let Some(knowledge) = &self.knowledge {
			knowledge.mark_dirty(uri);
		}

		Ok(response)
	}

	/// Handle a `BufferSyncClose` request.
	///
	/// Decrements the session's refcount. If the closing session was the owner,
	/// elects a successor (min session id) and bumps the epoch. If no sessions
	/// remain, removes the entry entirely.
	pub fn on_buffer_sync_close(
		self: &Arc<Self>,
		session_id: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = Self::normalize_uri(uri_in)?;
		let maybe_broadcast = {
			let mut sync = self.sync.lock().unwrap();
			let doc = sync
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			match doc.remove_open(session_id) {
				super::RemoveOpenResult::NotParticipant => return Err(ErrorCode::SyncDocNotFound),
				super::RemoveOpenResult::Removed => {
					if doc.participants.is_empty() {
						sync.sync_docs.remove(&uri);
						return Ok(ResponsePayload::BufferSyncClosed);
					}
					if session_id == doc.owner {
						let new_owner = doc.participants[0];
						doc.owner = new_owner;
						doc.epoch = SyncEpoch(doc.epoch.0 + 1);
						doc.seq = SyncSeq(0);
						doc.owner_needs_resync = true;
						let event = Event::BufferSyncOwnerChanged {
							uri,
							epoch: doc.epoch,
							owner: new_owner,
						};
						let sessions = doc.participants.clone();
						Some((event, sessions))
					} else {
						None
					}
				}
				super::RemoveOpenResult::Decremented => None,
			}
		};

		if let Some((event, sessions)) = maybe_broadcast {
			self.broadcast_to_sync_doc_sessions(&sessions, event, None);
		}

		Ok(ResponsePayload::BufferSyncClosed)
	}

	/// Handle a `BufferSyncDelta` request from the document owner.
	///
	/// Validates ownership and sequence ordering, applies the delta to the
	/// broker's authoritative rope, increments the sequence, and broadcasts
	/// the delta to all follower sessions.
	pub fn on_buffer_sync_delta(
		self: &Arc<Self>,
		session_id: SessionId,
		uri_in: &str,
		epoch: SyncEpoch,
		base_seq: SyncSeq,
		wire_tx: &WireTx,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = Self::normalize_uri(uri_in)?;
		let (new_seq, event, sessions) = {
			let mut sync = self.sync.lock().unwrap();
			let doc = sync
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			if session_id != doc.owner {
				return Err(ErrorCode::NotDocOwner);
			}
			if epoch != doc.epoch {
				return Err(ErrorCode::SyncEpochMismatch);
			}
			if base_seq != doc.seq {
				doc.owner_needs_resync = true;
				return Err(ErrorCode::SyncSeqMismatch);
			}
			if doc.owner_needs_resync {
				return Err(ErrorCode::OwnerNeedsResync);
			}

			let tx = crate::wire_convert::wire_to_tx(wire_tx, doc.rope.slice(..))
				.map_err(|_| ErrorCode::InvalidDelta)?;
			tx.apply(&mut doc.rope);
			doc.seq = SyncSeq(doc.seq.0 + 1);

			let event = Event::BufferSyncDelta {
				uri: uri.clone(),
				epoch: doc.epoch,
				seq: doc.seq,
				tx: wire_tx.clone(),
			};
			let sessions = doc.participants.clone();
			(doc.seq, event, sessions)
		};

		self.broadcast_to_sync_doc_sessions(&sessions, event, Some(session_id));

		if let Some(knowledge) = &self.knowledge {
			knowledge.mark_dirty(uri);
		}

		Ok(ResponsePayload::BufferSyncDeltaAck { seq: new_seq })
	}

	/// Handle a `BufferSyncTakeOwnership` request.
	///
	/// Transfers ownership to the requesting session if it is the minimum
	/// session ID among participants, bumps the epoch, resets the sequence,
	/// and broadcasts the ownership change.
	pub fn on_buffer_sync_take_ownership(
		self: &Arc<Self>,
		session_id: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = Self::normalize_uri(uri_in)?;
		let (new_epoch, broadcast) = {
			let mut sync = self.sync.lock().unwrap();
			let doc = sync
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			if !doc.open_refcounts.contains_key(&session_id) {
				return Err(ErrorCode::SyncDocNotFound);
			}

			if session_id == doc.owner {
				return Ok(ResponsePayload::BufferSyncOwnership { epoch: doc.epoch });
			}

			let preferred_owner = doc.participants[0];
			if session_id != preferred_owner {
				return Ok(ResponsePayload::BufferSyncOwnership { epoch: doc.epoch });
			}

			doc.owner = session_id;
			doc.epoch = SyncEpoch(doc.epoch.0 + 1);
			doc.seq = SyncSeq(0);
			doc.owner_needs_resync = true;

			let event = Event::BufferSyncOwnerChanged {
				uri,
				epoch: doc.epoch,
				owner: session_id,
			};
			let sessions = doc.participants.clone();
			(doc.epoch, Some((event, sessions)))
		};

		if let Some((event, sessions)) = broadcast {
			self.broadcast_to_sync_doc_sessions(&sessions, event, None);
		}

		Ok(ResponsePayload::BufferSyncOwnership { epoch: new_epoch })
	}

	/// Handle a `BufferSyncResync` request.
	///
	/// Returns the full current snapshot of the document.
	pub fn on_buffer_sync_resync(
		&self,
		session_id: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = Self::normalize_uri(uri_in)?;
		let (rope, epoch, seq, owner) = {
			let mut sync = self.sync.lock().unwrap();
			let doc = sync
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			if !doc.open_refcounts.contains_key(&session_id) {
				return Err(ErrorCode::SyncDocNotFound);
			}

			if session_id == doc.owner {
				doc.owner_needs_resync = false;
			}

			(doc.rope.clone(), doc.epoch, doc.seq, doc.owner)
		};

		Ok(ResponsePayload::BufferSyncSnapshot {
			text: rope.to_string(),
			epoch,
			seq,
			owner,
		})
	}

	/// Broadcast an event to sessions participating in a sync document.
	///
	/// Sends the event to all provided sessions except `exclude_session`.
	/// Cleans up any sessions where the send fails.
	fn broadcast_to_sync_doc_sessions(
		self: &Arc<Self>,
		sessions: &[SessionId],
		event: Event,
		exclude_session: Option<SessionId>,
	) {
		let frame = IpcFrame::Event(event);
		let mut failed_sessions = Vec::new();

		for &sid in sessions {
			if Some(sid) == exclude_session {
				continue;
			}
			if !self.send_event(sid, frame.clone()) {
				failed_sessions.push(sid);
			}
		}

		if !failed_sessions.is_empty() {
			let core = self.clone();
			tokio::spawn(async move {
				for session_id in failed_sessions {
					core.handle_session_send_failure(session_id);
				}
			});
		}
	}

	/// Clean up all buffer sync documents when a session disconnects.
	///
	/// Removes the session from all sync doc refcounts. If the session was the
	/// owner, elects a successor and broadcasts the ownership change. If no
	/// sessions remain, removes the document entry.
	pub fn cleanup_session_sync_docs(self: &Arc<Self>, session_id: SessionId) {
		let mut broadcasts = Vec::new();

		{
			let mut sync = self.sync.lock().unwrap();
			let uris: Vec<String> = sync
				.sync_docs
				.iter()
				.filter(|(_, doc)| doc.open_refcounts.contains_key(&session_id))
				.map(|(uri, _)| uri.clone())
				.collect();

			for uri in uris {
				let doc = sync.sync_docs.get_mut(&uri).unwrap();
				doc.remove_participant_all(session_id);

				if doc.participants.is_empty() {
					sync.sync_docs.remove(&uri);
					continue;
				}

				if session_id == doc.owner {
					let new_owner = doc.participants[0];
					doc.owner = new_owner;
					doc.epoch = SyncEpoch(doc.epoch.0 + 1);
					doc.seq = SyncSeq(0);
					doc.owner_needs_resync = true;
					let event = Event::BufferSyncOwnerChanged {
						uri,
						epoch: doc.epoch,
						owner: new_owner,
					};
					let sessions = doc.participants.clone();
					broadcasts.push((event, sessions));
				}
			}
		}

		for (event, sessions) in broadcasts {
			self.broadcast_to_sync_doc_sessions(&sessions, event, None);
		}
	}
}
