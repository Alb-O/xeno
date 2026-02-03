//! Document synchronization service with single-writer enforcement and idle unlocks.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use ropey::Rope;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, Instant, interval};
use xeno_broker_proto::types::{
	BufferSyncOwnerConfirmStatus, BufferSyncOwnershipStatus, BufferSyncRole, ErrorCode, Event,
	ResponsePayload, SessionId, SyncEpoch, SyncSeq, WireTx,
};

use crate::wire_convert;

/// Maximum number of operations allowed in a single wire transaction.
const MAX_WIRE_TX_OPS: usize = 100_000;
/// Maximum bytes allowed for string inserts in a single wire transaction.
const MAX_INSERT_BYTES: usize = 8 * 1024 * 1024;
/// Poll interval for owner idle detection.
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Duration after which an owner is considered idle.
pub(crate) const OWNER_IDLE_TIMEOUT: Duration = Duration::from_secs(2);

/// Commands for the buffer sync service actor.
#[derive(Debug)]
pub enum BufferSyncCmd {
	/// Open a document or join an existing session.
	Open {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Initial text content (if creating).
		text: String,
		/// Optional version hint from the client.
		version_hint: Option<u32>,
		/// Reply channel for the opened state.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Decrement reference count for a session on a document.
	Close {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Reply channel for confirmation.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Apply an edit delta from the document owner.
	Delta {
		/// The session identity (must be owner).
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Current ownership era.
		epoch: SyncEpoch,
		/// Base sequence number this delta applies to.
		base_seq: SyncSeq,
		/// The transaction data.
		tx: WireTx,
		/// Reply channel for the new sequence number.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Update activity timestamp for a document.
	Activity {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Reply channel for acknowledgment.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Transition the session to the writer role.
	TakeOwnership {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Reply channel for the new epoch.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Prove local content alignment for a new owner.
	OwnerConfirm {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Expected ownership epoch.
		epoch: SyncEpoch,
		/// Length of the document in characters.
		len_chars: u64,
		/// 64-bit hash of the document content.
		hash64: u64,
		/// Reply channel for the confirmation result.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Fetch a full snapshot of the authoritative document.
	Resync {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Reply channel for the full snapshot.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Signal that a session has disconnected unexpectedly.
	SessionLost {
		/// The lost session identity.
		sid: SessionId,
	},
	/// Internal request for a document snapshot triad (epoch, seq, rope).
	Snapshot {
		/// Canonical document URI.
		uri: String,
		/// Reply channel for the triad.
		reply: oneshot::Sender<Option<(SyncEpoch, SyncSeq, Rope)>>,
	},
	/// Verifies if a document is currently active in the broker.
	IsOpen {
		/// Canonical document URI.
		uri: String,
		/// Reply channel for existence check.
		reply: oneshot::Sender<bool>,
	},
}

/// Handle for communicating with the [`BufferSyncService`].
#[derive(Clone, Debug)]
pub struct BufferSyncHandle {
	tx: mpsc::Sender<BufferSyncCmd>,
}

impl BufferSyncHandle {
	/// Wraps a command sender in a typed handle.
	pub fn new(tx: mpsc::Sender<BufferSyncCmd>) -> Self {
		Self { tx }
	}

	/// Opens a document URI.
	pub async fn open(
		&self,
		sid: SessionId,
		uri: String,
		text: String,
		version_hint: Option<u32>,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(BufferSyncCmd::Open {
				sid,
				uri,
				text,
				version_hint,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Closes a document URI.
	pub async fn close(&self, sid: SessionId, uri: String) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(BufferSyncCmd::Close { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Submits an edit delta.
	pub async fn delta(
		&self,
		sid: SessionId,
		uri: String,
		epoch: SyncEpoch,
		base_seq: SyncSeq,
		tx: WireTx,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(BufferSyncCmd::Delta {
				sid,
				uri,
				epoch,
				base_seq,
				tx,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Records local activity for a document.
	pub async fn activity(
		&self,
		sid: SessionId,
		uri: String,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(BufferSyncCmd::Activity { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Transitions to owner role.
	pub async fn take_ownership(
		&self,
		sid: SessionId,
		uri: String,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(BufferSyncCmd::TakeOwnership { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Confirms ownership alignment.
	pub async fn owner_confirm(
		&self,
		sid: SessionId,
		uri: String,
		epoch: SyncEpoch,
		len_chars: u64,
		hash64: u64,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(BufferSyncCmd::OwnerConfirm {
				sid,
				uri,
				epoch,
				len_chars,
				hash64,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Requests full content snapshot.
	pub async fn resync(&self, sid: SessionId, uri: String) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(BufferSyncCmd::Resync { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Cleans up a lost session.
	pub async fn session_lost(&self, sid: SessionId) {
		let _ = self.tx.send(BufferSyncCmd::SessionLost { sid }).await;
	}

	/// Returns a consistent snapshot of the document rope.
	pub async fn snapshot(&self, uri: String) -> Option<(SyncEpoch, SyncSeq, Rope)> {
		let (reply, rx) = oneshot::channel();
		let _ = self.tx.send(BufferSyncCmd::Snapshot { uri, reply }).await;
		rx.await.ok().flatten()
	}

	/// Verifies if a document is tracked.
	pub async fn is_open(&self, uri: String) -> bool {
		let (reply, rx) = oneshot::channel();
		if self
			.tx
			.send(BufferSyncCmd::IsOpen { uri, reply })
			.await
			.is_err()
		{
			return false;
		}
		rx.await.unwrap_or(false)
	}
}

/// State for a single synchronized document.
#[derive(Debug)]
struct SyncDocState {
	/// Current owner session permitted to submit deltas.
	owner: Option<SessionId>,
	/// Per-session reference counts.
	open_refcounts: HashMap<SessionId, u32>,
	/// Sorted list of all active participants for consistent broadcasting.
	participants: Vec<SessionId>,
	/// Last recorded activity timestamp per session.
	last_active: HashMap<SessionId, Instant>,
	/// Authoritative ownership era. Bumps on every writer change.
	epoch: SyncEpoch,
	/// Authoritative edit sequence. Bumps on every edit; resets to 0 on epoch change.
	seq: SyncSeq,
	/// The actual text content.
	rope: Rope,
	/// Cached 64-bit hash of the document content.
	hash64: u64,
	/// Cached length of the document in characters.
	len_chars: u64,
	/// Flag indicating the writer must perform a full resync before publishing.
	owner_needs_resync: bool,
}

impl SyncDocState {
	fn update_fingerprint(&mut self) {
		let (len, hash) = xeno_broker_proto::fingerprint_rope(&self.rope);
		self.len_chars = len;
		self.hash64 = hash;
	}

	fn add_open(&mut self, sid: SessionId) {
		let count = self.open_refcounts.entry(sid).or_insert(0);
		if *count == 0
			&& let Err(idx) = self.participants.binary_search(&sid)
		{
			self.participants.insert(idx, sid);
		}
		*count += 1;
		self.touch(sid);
	}

	fn remove_open(&mut self, sid: SessionId) -> RemoveOpenResult {
		let Some(count) = self.open_refcounts.get_mut(&sid) else {
			return RemoveOpenResult::NotParticipant;
		};

		if *count > 1 {
			*count -= 1;
			RemoveOpenResult::Decremented
		} else {
			self.open_refcounts.remove(&sid);
			self.last_active.remove(&sid);
			if let Ok(idx) = self.participants.binary_search(&sid) {
				self.participants.remove(idx);
			}
			RemoveOpenResult::Removed
		}
	}

	fn remove_participant_all(&mut self, sid: SessionId) -> bool {
		if self.open_refcounts.remove(&sid).is_some() {
			self.last_active.remove(&sid);
			if let Ok(idx) = self.participants.binary_search(&sid) {
				self.participants.remove(idx);
			}
			true
		} else {
			false
		}
	}

	fn touch(&mut self, sid: SessionId) {
		self.last_active.insert(sid, Instant::now());
	}

	fn owner_idle(&self, now: Instant) -> bool {
		let Some(owner) = self.owner else {
			return false;
		};
		let Some(last) = self.last_active.get(&owner) else {
			return true;
		};
		now.duration_since(*last) >= OWNER_IDLE_TIMEOUT
	}
}

#[derive(Debug, PartialEq, Eq)]
enum RemoveOpenResult {
	Decremented,
	Removed,
	NotParticipant,
}

/// Actor service managing single-writer document consistency.
///
/// Implements a multi-session synchronization protocol where the broker holds
/// the authoritative copy of the text. One session is elected as the writer (Owner),
/// and all other sessions receive broadcasted deltas as read-only followers. When
/// a document is unlocked it is "up-for-grabs": the first editor to claim ownership
/// becomes the sole writer until it releases or idles out.
pub struct BufferSyncService {
	rx: mpsc::Receiver<BufferSyncCmd>,
	sync_docs: HashMap<String, SyncDocState>,
	/// Shared set of open URIs exposed to the knowledge crawler.
	open_docs_set: Arc<Mutex<HashSet<String>>>,
	sessions: super::sessions::SessionHandle,
	knowledge: Option<super::knowledge::KnowledgeHandle>,
}

impl BufferSyncService {
	/// Spawns the buffer sync service actor.
	pub fn start(
		sessions: super::sessions::SessionHandle,
	) -> (
		BufferSyncHandle,
		Arc<Mutex<HashSet<String>>>,
		mpsc::Sender<super::knowledge::KnowledgeHandle>,
	) {
		let (tx, rx) = mpsc::channel(256);
		let (knowledge_tx, knowledge_rx) = mpsc::channel(1);
		let open_docs_set = Arc::new(Mutex::new(HashSet::new()));

		let service = Self {
			rx,
			sync_docs: HashMap::new(),
			open_docs_set: open_docs_set.clone(),
			sessions,
			knowledge: None,
		};

		tokio::spawn(service.run(knowledge_rx));

		(BufferSyncHandle::new(tx), open_docs_set, knowledge_tx)
	}

	async fn run(mut self, mut knowledge_rx: mpsc::Receiver<super::knowledge::KnowledgeHandle>) {
		if let Some(h) = knowledge_rx.recv().await {
			self.knowledge = Some(h);
		}

		let mut idle_tick = interval(IDLE_POLL_INTERVAL);

		loop {
			tokio::select! {
				cmd = self.rx.recv() => {
					let Some(cmd) = cmd else {
						break;
					};
					match cmd {
						BufferSyncCmd::Open {
							sid,
							uri,
							text,
							version_hint: _,
							reply,
						} => {
							let result = self.handle_open(sid, &uri, &text);
							let _ = reply.send(result);
						}
						BufferSyncCmd::Close { sid, uri, reply } => {
							let result = self.handle_close(sid, &uri).await;
							let _ = reply.send(result);
						}
						BufferSyncCmd::Delta {
							sid,
							uri,
							epoch,
							base_seq,
							tx,
							reply,
						} => {
							let result = self.handle_delta(sid, &uri, epoch, base_seq, &tx).await;
							let _ = reply.send(result);
						}
						BufferSyncCmd::Activity { sid, uri, reply } => {
							let result = self.handle_activity(sid, &uri);
							let _ = reply.send(result);
						}
						BufferSyncCmd::TakeOwnership { sid, uri, reply } => {
							let result = self.handle_take_ownership(sid, &uri).await;
							let _ = reply.send(result);
						}
						BufferSyncCmd::OwnerConfirm {
							sid,
							uri,
							epoch,
							len_chars,
							hash64,
							reply,
						} => {
							let result = self.handle_owner_confirm(sid, &uri, epoch, len_chars, hash64);
							let _ = reply.send(result);
						}
						BufferSyncCmd::Resync { sid, uri, reply } => {
							let result = self.handle_resync(sid, &uri).await;
							let _ = reply.send(result);
						}
						BufferSyncCmd::SessionLost { sid } => {
							self.handle_session_cleanup(sid).await;
						}
						BufferSyncCmd::Snapshot { uri, reply } => {
							let snapshot = self
								.sync_docs
								.get(&uri)
								.map(|doc| (doc.epoch, doc.seq, doc.rope.clone()));
							let _ = reply.send(snapshot);
						}
						BufferSyncCmd::IsOpen { uri, reply } => {
							let _ = reply.send(self.sync_docs.contains_key(&uri));
						}
					}
				}
				_ = idle_tick.tick() => {
					self.handle_idle_tick().await;
				}
			}
		}
	}

	fn handle_open(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		text: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;

		let (role, epoch, seq, snapshot_rope) = match self.sync_docs.get_mut(&uri) {
			None => {
				let mut doc = SyncDocState {
					owner: Some(sid),
					open_refcounts: HashMap::new(),
					participants: Vec::new(),
					last_active: HashMap::new(),
					epoch: SyncEpoch(1),
					seq: SyncSeq(0),
					rope: Rope::from(text),
					hash64: 0,
					len_chars: 0,
					owner_needs_resync: false,
				};
				doc.update_fingerprint();
				doc.add_open(sid);
				self.sync_docs.insert(uri.clone(), doc);
				self.open_docs_set.lock().unwrap().insert(uri.clone());
				(BufferSyncRole::Owner, SyncEpoch(1), SyncSeq(0), None)
			}
			Some(doc) => {
				doc.add_open(sid);
				(
					BufferSyncRole::Follower,
					doc.epoch,
					doc.seq,
					Some(doc.rope.clone()),
				)
			}
		};

		if let Some(knowledge) = &self.knowledge {
			let _ = knowledge.doc_dirty(uri);
		}

		Ok(ResponsePayload::BufferSyncOpened {
			role,
			epoch,
			seq,
			snapshot: snapshot_rope.map(|r| r.to_string()),
		})
	}

	async fn handle_close(
		&mut self,
		sid: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let doc = self
			.sync_docs
			.get_mut(&uri)
			.ok_or(ErrorCode::SyncDocNotFound)?;
		let mut unlock = None;

		match doc.remove_open(sid) {
			RemoveOpenResult::NotParticipant => return Err(ErrorCode::SyncDocNotFound),
			RemoveOpenResult::Removed => {
				if doc.participants.is_empty() {
					self.sync_docs.remove(&uri);
					self.open_docs_set.lock().unwrap().remove(&uri);
					return Ok(ResponsePayload::BufferSyncClosed);
				}
				if doc.owner == Some(sid) {
					unlock = Some(Self::prepare_unlock(&uri, doc));
				}
			}
			RemoveOpenResult::Decremented => {}
		}

		if let Some((targets, event)) = unlock {
			self.sessions
				.broadcast(
					targets,
					xeno_broker_proto::types::IpcFrame::Event(event),
					None,
				)
				.await;
		}

		Ok(ResponsePayload::BufferSyncClosed)
	}

	async fn handle_delta(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		epoch: SyncEpoch,
		base_seq: SyncSeq,
		wire_tx: &WireTx,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let doc = self
			.sync_docs
			.get_mut(&uri)
			.ok_or(ErrorCode::SyncDocNotFound)?;

		if doc.owner != Some(sid) {
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

		if wire_tx.0.len() > MAX_WIRE_TX_OPS {
			return Err(ErrorCode::InvalidDelta);
		}
		let insert_bytes: usize = wire_tx
			.0
			.iter()
			.filter_map(|op| match op {
				xeno_broker_proto::types::WireOp::Insert(s) => Some(s.len()),
				_ => None,
			})
			.sum();
		if insert_bytes > MAX_INSERT_BYTES {
			return Err(ErrorCode::InvalidDelta);
		}

		let tx = wire_convert::wire_to_tx(wire_tx, doc.rope.slice(..))
			.map_err(|_| ErrorCode::InvalidDelta)?;

		tx.apply(&mut doc.rope);
		doc.seq = SyncSeq(doc.seq.0 + 1);
		doc.update_fingerprint();
		doc.touch(sid);

		let event = Event::BufferSyncDelta {
			uri: uri.clone(),
			epoch: doc.epoch,
			seq: doc.seq,
			tx: wire_tx.clone(),
		};

		self.sessions
			.broadcast(
				doc.participants.clone(),
				xeno_broker_proto::types::IpcFrame::Event(event),
				Some(sid),
			)
			.await;

		if let Some(knowledge) = &self.knowledge {
			let _ = knowledge.doc_dirty(uri);
		}

		Ok(ResponsePayload::BufferSyncDeltaAck { seq: doc.seq })
	}

	fn handle_activity(
		&mut self,
		sid: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let doc = self
			.sync_docs
			.get_mut(&uri)
			.ok_or(ErrorCode::SyncDocNotFound)?;

		if !doc.open_refcounts.contains_key(&sid) {
			return Err(ErrorCode::SyncDocNotFound);
		}

		doc.touch(sid);
		Ok(ResponsePayload::BufferSyncActivityAck)
	}

	async fn handle_idle_tick(&mut self) {
		let now = Instant::now();
		let mut unlocks = Vec::new();

		for (uri, doc) in &mut self.sync_docs {
			if doc.owner_idle(now) {
				unlocks.push(Self::prepare_unlock(uri, doc));
			}
		}

		for (targets, event) in unlocks {
			self.sessions
				.broadcast(
					targets,
					xeno_broker_proto::types::IpcFrame::Event(event),
					None,
				)
				.await;
		}
	}

	fn prepare_unlock(uri: &str, doc: &mut SyncDocState) -> (Vec<SessionId>, Event) {
		doc.owner = None;
		doc.epoch = SyncEpoch(doc.epoch.0 + 1);
		doc.seq = SyncSeq(0);
		doc.owner_needs_resync = true;

		let event = Event::BufferSyncUnlocked {
			uri: uri.to_string(),
			epoch: doc.epoch,
			hash64: doc.hash64,
			len_chars: doc.len_chars,
		};
		(doc.participants.clone(), event)
	}

	async fn handle_take_ownership(
		&mut self,
		sid: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let doc = self
			.sync_docs
			.get_mut(&uri)
			.ok_or(ErrorCode::SyncDocNotFound)?;

		if !doc.open_refcounts.contains_key(&sid) {
			return Err(ErrorCode::SyncDocNotFound);
		}
		if doc.owner == Some(sid) {
			doc.touch(sid);
			return Ok(ResponsePayload::BufferSyncOwnership {
				status: BufferSyncOwnershipStatus::AlreadyOwner,
				epoch: doc.epoch,
				owner: sid,
			});
		}
		if let Some(owner) = doc.owner {
			return Ok(ResponsePayload::BufferSyncOwnership {
				status: BufferSyncOwnershipStatus::Denied,
				epoch: doc.epoch,
				owner,
			});
		}

		doc.owner = Some(sid);
		doc.epoch = SyncEpoch(doc.epoch.0 + 1);
		doc.seq = SyncSeq(0);
		doc.owner_needs_resync = true;
		doc.touch(sid);

		let event = Event::BufferSyncOwnerChanged {
			uri,
			epoch: doc.epoch,
			owner: sid,
			hash64: doc.hash64,
			len_chars: doc.len_chars,
		};
		self.sessions
			.broadcast(
				doc.participants.clone(),
				xeno_broker_proto::types::IpcFrame::Event(event),
				None,
			)
			.await;

		Ok(ResponsePayload::BufferSyncOwnership {
			status: BufferSyncOwnershipStatus::Granted,
			epoch: doc.epoch,
			owner: sid,
		})
	}

	fn handle_owner_confirm(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		epoch: SyncEpoch,
		len_chars: u64,
		hash64: u64,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let doc = self
			.sync_docs
			.get_mut(&uri)
			.ok_or(ErrorCode::SyncDocNotFound)?;

		if doc.owner != Some(sid) {
			return Err(ErrorCode::NotDocOwner);
		}
		if epoch != doc.epoch {
			return Err(ErrorCode::SyncEpochMismatch);
		}

		doc.touch(sid);
		let owner = doc.owner.unwrap_or(sid);
		if len_chars == doc.len_chars && hash64 == doc.hash64 {
			doc.owner_needs_resync = false;
			Ok(ResponsePayload::BufferSyncOwnerConfirmResult {
				status: BufferSyncOwnerConfirmStatus::Confirmed,
				epoch: doc.epoch,
				seq: doc.seq,
				owner,
				snapshot: None,
			})
		} else {
			Ok(ResponsePayload::BufferSyncOwnerConfirmResult {
				status: BufferSyncOwnerConfirmStatus::NeedSnapshot,
				epoch: doc.epoch,
				seq: doc.seq,
				owner,
				snapshot: Some(doc.rope.to_string()),
			})
		}
	}

	async fn handle_resync(
		&mut self,
		sid: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let doc = self
			.sync_docs
			.get_mut(&uri)
			.ok_or(ErrorCode::SyncDocNotFound)?;

		if !doc.open_refcounts.contains_key(&sid) {
			return Err(ErrorCode::SyncDocNotFound);
		}
		doc.touch(sid);
		if doc.owner == Some(sid) {
			doc.owner_needs_resync = false;
		}

		Ok(ResponsePayload::BufferSyncSnapshot {
			text: doc.rope.to_string(),
			epoch: doc.epoch,
			seq: doc.seq,
			owner: doc.owner,
		})
	}

	async fn handle_session_cleanup(&mut self, sid: SessionId) {
		let uris: Vec<String> = self
			.sync_docs
			.iter()
			.filter(|(_, doc)| doc.open_refcounts.contains_key(&sid))
			.map(|(uri, _)| uri.clone())
			.collect();

		for uri in uris {
			let mut unlock = None;
			if let Some(doc) = self.sync_docs.get_mut(&uri) {
				doc.remove_participant_all(sid);
				if doc.participants.is_empty() {
					self.sync_docs.remove(&uri);
					self.open_docs_set.lock().unwrap().remove(&uri);
					continue;
				}
				if doc.owner == Some(sid) {
					unlock = Some(Self::prepare_unlock(&uri, doc));
				}
			}
			if let Some((targets, event)) = unlock {
				self.sessions
					.broadcast(
						targets,
						xeno_broker_proto::types::IpcFrame::Event(event),
						None,
					)
					.await;
			}
		}
	}
}
