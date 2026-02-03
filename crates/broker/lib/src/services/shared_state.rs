//! Shared document state service with preferred-owner enforcement and idle unlocks.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use helix_db::helix_engine::storage_core::HelixGraphStorage;
use ropey::Rope;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, Instant, interval};
use xeno_broker_proto::types::{
	DocStateSnapshot, DocSyncPhase, ErrorCode, Event, ResponsePayload, SessionId, SyncEpoch,
	SyncSeq, WireTx,
};

use crate::core::history::{HistoryMeta, HistoryStore};
use crate::wire_convert;

/// Maximum number of operations allowed in a single wire transaction.
const MAX_WIRE_TX_OPS: usize = 100_000;
/// Maximum bytes allowed for string inserts in a single wire transaction.
const MAX_INSERT_BYTES: usize = 8 * 1024 * 1024;
/// Poll interval for owner idle detection.
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Duration after which an owner is considered idle.
pub(crate) const OWNER_IDLE_TIMEOUT: Duration = Duration::from_secs(2);
/// Maximum number of history nodes retained per document.
const MAX_HISTORY_NODES: usize = 100;

/// Commands for the shared state service actor.
#[derive(Debug)]
pub enum SharedStateCmd {
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
	Edit {
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
	/// Update focus status for a document.
	Focus {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Whether the session is focused on the document.
		focused: bool,
		/// Monotonic sequence number for focus transitions.
		focus_seq: u64,
		/// Reply channel for the updated snapshot.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Fetch a full snapshot of the authoritative document.
	Resync {
		/// The session identity.
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Optional hash of the client's current content.
		client_hash64: Option<u64>,
		/// Optional length of the client's current content.
		client_len_chars: Option<u64>,
		/// Reply channel for the full snapshot.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Undo the last change for a document.
	Undo {
		/// The session identity (must be owner).
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Reply channel for acknowledgment.
		reply: oneshot::Sender<Result<ResponsePayload, ErrorCode>>,
	},
	/// Redo the last undone change for a document.
	Redo {
		/// The session identity (must be owner).
		sid: SessionId,
		/// Canonical document URI.
		uri: String,
		/// Reply channel for acknowledgment.
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

/// Handle for communicating with the [`SharedStateService`].
#[derive(Clone, Debug)]
pub struct SharedStateHandle {
	tx: mpsc::Sender<SharedStateCmd>,
}

impl SharedStateHandle {
	/// Wraps a command sender in a typed handle.
	pub fn new(tx: mpsc::Sender<SharedStateCmd>) -> Self {
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
			.send(SharedStateCmd::Open {
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
			.send(SharedStateCmd::Close { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Submits an edit delta.
	pub async fn edit(
		&self,
		sid: SessionId,
		uri: String,
		epoch: SyncEpoch,
		base_seq: SyncSeq,
		tx: WireTx,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Edit {
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
			.send(SharedStateCmd::Activity { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Updates focus status for a document.
	pub async fn focus(
		&self,
		sid: SessionId,
		uri: String,
		focused: bool,
		focus_seq: u64,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Focus {
				sid,
				uri,
				focused,
				focus_seq,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Requests full content snapshot.
	pub async fn resync(
		&self,
		sid: SessionId,
		uri: String,
		client_hash64: Option<u64>,
		client_len_chars: Option<u64>,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Resync {
				sid,
				uri,
				client_hash64,
				client_len_chars,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Requests undo for a shared document.
	pub async fn undo(&self, sid: SessionId, uri: String) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Undo { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Requests redo for a shared document.
	pub async fn redo(&self, sid: SessionId, uri: String) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Redo { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Cleans up a lost session.
	pub async fn session_lost(&self, sid: SessionId) {
		let _ = self.tx.send(SharedStateCmd::SessionLost { sid }).await;
	}

	/// Returns a consistent snapshot of the document rope.
	pub async fn snapshot(&self, uri: String) -> Option<(SyncEpoch, SyncSeq, Rope)> {
		let (reply, rx) = oneshot::channel();
		let _ = self.tx.send(SharedStateCmd::Snapshot { uri, reply }).await;
		rx.await.ok().flatten()
	}

	/// Verifies if a document is tracked.
	pub async fn is_open(&self, uri: String) -> bool {
		let (reply, rx) = oneshot::channel();
		if self
			.tx
			.send(SharedStateCmd::IsOpen { uri, reply })
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
	/// Preferred owner session (focused editor).
	preferred_owner: Option<SessionId>,
	/// Per-session reference counts.
	open_refcounts: HashMap<SessionId, u32>,
	/// Sorted list of all active participants for consistent broadcasting.
	participants: Vec<SessionId>,
	/// Last recorded activity timestamp per session.
	last_active: HashMap<SessionId, Instant>,
	/// Last recorded focus sequence per session.
	last_focus_seq: HashMap<SessionId, u64>,
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
	/// History metadata for broker-owned undo/redo.
	history: Option<HistoryMeta>,
	/// Flag indicating the writer must perform a full resync before publishing.
	owner_needs_resync: bool,
}

impl SyncDocState {
	fn snapshot(&self, uri: &str) -> DocStateSnapshot {
		let phase = if self.owner.is_none() {
			DocSyncPhase::Unlocked
		} else if self.owner_needs_resync {
			DocSyncPhase::Diverged
		} else {
			DocSyncPhase::Owned
		};
		DocStateSnapshot {
			uri: uri.to_string(),
			epoch: self.epoch,
			seq: self.seq,
			owner: self.owner,
			preferred_owner: self.preferred_owner,
			phase,
			hash64: self.hash64,
			len_chars: self.len_chars,
		}
	}

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
			self.last_focus_seq.remove(&sid);
			if let Ok(idx) = self.participants.binary_search(&sid) {
				self.participants.remove(idx);
			}
			RemoveOpenResult::Removed
		}
	}

	fn remove_participant_all(&mut self, sid: SessionId) -> bool {
		if self.open_refcounts.remove(&sid).is_some() {
			self.last_active.remove(&sid);
			self.last_focus_seq.remove(&sid);
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

/// Actor service managing shared document consistency.
///
/// The broker holds the authoritative copy of the text. One session is elected
/// as the current owner, and a preferred owner (focused editor) is allowed to
/// publish deltas. Ownership changes are driven by focus events; new owners must
/// resync before publishing edits. Unlocks are broadcast on idle, blur, or disconnect.
pub struct SharedStateService {
	rx: mpsc::Receiver<SharedStateCmd>,
	sync_docs: HashMap<String, SyncDocState>,
	history: Option<HistoryStore>,
	/// Shared set of open URIs exposed to the knowledge crawler.
	open_docs_set: Arc<Mutex<HashSet<String>>>,
	sessions: super::sessions::SessionHandle,
	knowledge: Option<super::knowledge::KnowledgeHandle>,
	routing: Option<super::routing::RoutingHandle>,
}

impl SharedStateService {
	/// Spawns the shared state service actor.
	pub fn start(
		sessions: super::sessions::SessionHandle,
		storage: Option<Arc<HelixGraphStorage>>,
	) -> (
		SharedStateHandle,
		Arc<Mutex<HashSet<String>>>,
		mpsc::Sender<super::knowledge::KnowledgeHandle>,
		mpsc::Sender<super::routing::RoutingHandle>,
	) {
		let (tx, rx) = mpsc::channel(256);
		let (knowledge_tx, knowledge_rx) = mpsc::channel(1);
		let (routing_tx, routing_rx) = mpsc::channel(1);
		let open_docs_set = Arc::new(Mutex::new(HashSet::new()));

		let service = Self {
			rx,
			sync_docs: HashMap::new(),
			history: storage.map(HistoryStore::new),
			open_docs_set: open_docs_set.clone(),
			sessions,
			knowledge: None,
			routing: None,
		};

		tokio::spawn(service.run(knowledge_rx, routing_rx));

		(
			SharedStateHandle::new(tx),
			open_docs_set,
			knowledge_tx,
			routing_tx,
		)
	}

	async fn run(
		mut self,
		mut knowledge_rx: mpsc::Receiver<super::knowledge::KnowledgeHandle>,
		mut routing_rx: mpsc::Receiver<super::routing::RoutingHandle>,
	) {
		if let Some(h) = knowledge_rx.recv().await {
			self.knowledge = Some(h);
		}
		if let Some(h) = routing_rx.recv().await {
			self.routing = Some(h);
		}

		let mut idle_tick = interval(IDLE_POLL_INTERVAL);

		loop {
			tokio::select! {
				cmd = self.rx.recv() => {
					let Some(cmd) = cmd else {
						break;
					};
					match cmd {
						SharedStateCmd::Open {
							sid,
							uri,
							text,
							version_hint: _,
							reply,
						} => {
							let result = self.handle_open(sid, &uri, &text).await;
							let _ = reply.send(result);
						}
						SharedStateCmd::Close { sid, uri, reply } => {
							let result = self.handle_close(sid, &uri).await;
							let _ = reply.send(result);
						}
						SharedStateCmd::Edit {
							sid,
							uri,
							epoch,
							base_seq,
							tx,
							reply,
						} => {
							let result = self.handle_edit(sid, &uri, epoch, base_seq, &tx).await;
							let _ = reply.send(result);
						}
						SharedStateCmd::Activity { sid, uri, reply } => {
							let result = self.handle_activity(sid, &uri);
							let _ = reply.send(result);
						}
						SharedStateCmd::Focus {
							sid,
							uri,
							focused,
							focus_seq,
							reply,
						} => {
							let result = self.handle_focus(sid, &uri, focused, focus_seq).await;
							let _ = reply.send(result);
						}
						SharedStateCmd::Resync {
							sid,
							uri,
							client_hash64,
							client_len_chars,
							reply,
						} => {
							let result = self.handle_resync(sid, &uri, client_hash64, client_len_chars).await;
							let _ = reply.send(result);
						}
						SharedStateCmd::Undo { sid, uri, reply } => {
							let result = self.handle_undo(sid, &uri).await;
							let _ = reply.send(result);
						}
						SharedStateCmd::Redo { sid, uri, reply } => {
							let result = self.handle_redo(sid, &uri).await;
							let _ = reply.send(result);
						}
						SharedStateCmd::SessionLost { sid } => {
							self.handle_session_cleanup(sid).await;
						}
						SharedStateCmd::Snapshot { uri, reply } => {
							let snapshot = self
								.sync_docs
								.get(&uri)
								.map(|doc| (doc.epoch, doc.seq, doc.rope.clone()));
							let _ = reply.send(snapshot);
						}
						SharedStateCmd::IsOpen { uri, reply } => {
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

	async fn handle_open(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		text: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let mut routing_open = None;
		let (snapshot, text) = match self.sync_docs.get_mut(&uri) {
			None => {
				let mut doc = SyncDocState {
					owner: Some(sid),
					preferred_owner: Some(sid),
					open_refcounts: HashMap::new(),
					participants: Vec::new(),
					last_active: HashMap::new(),
					last_focus_seq: HashMap::new(),
					epoch: SyncEpoch(1),
					seq: SyncSeq(0),
					rope: Rope::new(),
					hash64: 0,
					len_chars: 0,
					history: None,
					owner_needs_resync: false,
				};

				let mut send_text = None;
				if let Some(history) = &self.history {
					match history.load_doc(&uri) {
						Ok(Some(stored)) => {
							doc.epoch = stored.epoch;
							doc.seq = stored.seq;
							doc.rope = stored.rope;
							doc.hash64 = stored.hash64;
							doc.len_chars = stored.len_chars;
							doc.history = Some(stored.meta);
							send_text = Some(doc.rope.to_string());
						}
						Ok(None) => {
							let rope = Rope::from(text);
							let (len, hash) = xeno_broker_proto::fingerprint_rope(&rope);
							let stored = history
								.create_doc(&uri, &rope, doc.epoch, doc.seq, hash, len)
								.map_err(|err| {
									tracing::warn!(error = %err, ?uri, "history create failed");
									ErrorCode::Internal
								})?;
							doc.rope = stored.rope;
							doc.hash64 = stored.hash64;
							doc.len_chars = stored.len_chars;
							doc.history = Some(stored.meta);
						}
						Err(err) => {
							tracing::warn!(error = %err, ?uri, "history load failed");
							return Err(ErrorCode::Internal);
						}
					}
				} else {
					doc.rope = Rope::from(text);
					doc.update_fingerprint();
				}

				doc.add_open(sid);
				let snapshot = doc.snapshot(&uri);
				routing_open = Some(doc.rope.to_string());
				self.sync_docs.insert(uri.clone(), doc);
				self.open_docs_set.lock().unwrap().insert(uri.clone());
				(snapshot, send_text)
			}
			Some(doc) => {
				doc.add_open(sid);
				if doc.preferred_owner.is_none() {
					doc.preferred_owner = Some(sid);
				}
				let text = (doc.owner != Some(sid)).then(|| doc.rope.to_string());
				let snapshot = doc.snapshot(&uri);
				(snapshot, text)
			}
		};

		if let Some(knowledge) = &self.knowledge {
			let _ = knowledge.doc_dirty(uri.clone());
		}
		if let Some(text) = routing_open {
			self.notify_lsp_open(uri.clone(), text).await;
		}

		Ok(ResponsePayload::SharedOpened { snapshot, text })
	}

	async fn handle_close(
		&mut self,
		sid: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let mut unlock = None;
		let mut closed = false;
		let history = self.history.as_ref();

		{
			let doc = self
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			match doc.remove_open(sid) {
				RemoveOpenResult::NotParticipant => return Err(ErrorCode::SyncDocNotFound),
				RemoveOpenResult::Removed => {
					if doc.participants.is_empty() {
						closed = true;
					} else if doc.owner == Some(sid) {
						unlock = Some(Self::prepare_unlock(history, &uri, doc));
					}
				}
				RemoveOpenResult::Decremented => {}
			}
		}

		if closed {
			self.sync_docs.remove(&uri);
			self.open_docs_set.lock().unwrap().remove(&uri);
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
		if closed {
			self.notify_lsp_close(uri).await;
		}

		Ok(ResponsePayload::SharedClosed)
	}

	async fn handle_edit(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		epoch: SyncEpoch,
		base_seq: SyncSeq,
		wire_tx: &WireTx,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let (event, lsp_text, participants, seq, final_epoch) = {
			let doc = self
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			if !doc.open_refcounts.contains_key(&sid) {
				return Err(ErrorCode::SyncDocNotFound);
			}
			if let Some(preferred) = doc.preferred_owner
				&& preferred != sid
			{
				return Err(ErrorCode::NotPreferredOwner);
			}
			if doc.owner != Some(sid) {
				return Err(ErrorCode::NotPreferredOwner);
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

			let pre_rope = doc.rope.clone();
			let tx = wire_convert::wire_to_tx(wire_tx, pre_rope.slice(..))
				.map_err(|_| ErrorCode::InvalidDelta)?;
			let undo_tx = tx.invert(&pre_rope);

			tx.apply(&mut doc.rope);
			doc.seq = SyncSeq(doc.seq.0 + 1);
			doc.update_fingerprint();
			doc.touch(sid);

			if let Some(history) = &self.history {
				let Some(meta) = doc.history.as_mut() else {
					tracing::warn!(?uri, "history metadata missing");
					return Err(ErrorCode::Internal);
				};
				let undo_wire = wire_convert::tx_to_wire(&undo_tx);
				let redo_wire = wire_tx.clone();
				history
					.append_edit(
						&uri,
						meta,
						doc.epoch,
						doc.seq,
						doc.hash64,
						doc.len_chars,
						redo_wire,
						undo_wire,
						MAX_HISTORY_NODES,
					)
					.map_err(|err| {
						tracing::warn!(error = %err, ?uri, "history append failed");
						ErrorCode::Internal
					})?;
			}

			let event = Event::SharedDelta {
				uri: uri.clone(),
				epoch: doc.epoch,
				seq: doc.seq,
				tx: wire_tx.clone(),
			};
			let lsp_text = doc.rope.to_string();
			let participants = doc.participants.clone();
			let seq = doc.seq;
			let final_epoch = doc.epoch;
			(event, lsp_text, participants, seq, final_epoch)
		};

		self.sessions
			.broadcast(
				participants,
				xeno_broker_proto::types::IpcFrame::Event(event),
				None,
			)
			.await;

		self.notify_lsp_update(uri.clone(), lsp_text).await;

		if let Some(knowledge) = &self.knowledge {
			let _ = knowledge.doc_dirty(uri);
		}

		Ok(ResponsePayload::SharedEditAck {
			epoch: final_epoch,
			seq,
		})
	}

	async fn handle_undo(
		&mut self,
		sid: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let (event, lsp_text, participants, seq, final_epoch) = {
			let doc = self
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			if !doc.open_refcounts.contains_key(&sid) {
				return Err(ErrorCode::SyncDocNotFound);
			}
			if let Some(preferred) = doc.preferred_owner
				&& preferred != sid
			{
				return Err(ErrorCode::NotPreferredOwner);
			}
			if doc.owner != Some(sid) {
				return Err(ErrorCode::NotPreferredOwner);
			}
			if doc.owner_needs_resync {
				return Err(ErrorCode::OwnerNeedsResync);
			}

			let Some(history) = &self.history else {
				return Err(ErrorCode::Internal);
			};
			let Some(meta) = doc.history.as_ref() else {
				return Err(ErrorCode::Internal);
			};
			let head_id = meta.head_id;

			let Some((parent_id, undo_wire)) = history.load_undo(&uri, head_id).map_err(|err| {
				tracing::warn!(error = %err, ?uri, "history undo load failed");
				ErrorCode::Internal
			})?
			else {
				return Err(ErrorCode::NothingToUndo);
			};

			let tx = wire_convert::wire_to_tx(&undo_wire, doc.rope.slice(..))
				.map_err(|_| ErrorCode::InvalidDelta)?;
			tx.apply(&mut doc.rope);
			doc.seq = SyncSeq(doc.seq.0 + 1);
			doc.update_fingerprint();
			doc.touch(sid);

			{
				let Some(meta) = doc.history.as_mut() else {
					return Err(ErrorCode::Internal);
				};
				meta.head_id = parent_id;
				history
					.update_doc_state(&uri, meta, doc.epoch, doc.seq, doc.hash64, doc.len_chars)
					.map_err(|err| {
						tracing::warn!(error = %err, ?uri, "history undo persist failed");
						ErrorCode::Internal
					})?;
			}

			let event = Event::SharedDelta {
				uri: uri.clone(),
				epoch: doc.epoch,
				seq: doc.seq,
				tx: undo_wire,
			};
			let lsp_text = doc.rope.to_string();
			let participants = doc.participants.clone();
			let seq = doc.seq;
			let final_epoch = doc.epoch;
			(event, lsp_text, participants, seq, final_epoch)
		};

		self.sessions
			.broadcast(
				participants,
				xeno_broker_proto::types::IpcFrame::Event(event),
				None,
			)
			.await;

		self.notify_lsp_update(uri.clone(), lsp_text).await;

		if let Some(knowledge) = &self.knowledge {
			let _ = knowledge.doc_dirty(uri);
		}

		Ok(ResponsePayload::SharedUndoAck {
			epoch: final_epoch,
			seq,
		})
	}

	async fn handle_redo(
		&mut self,
		sid: SessionId,
		uri_in: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let (event, lsp_text, participants, seq, final_epoch) = {
			let doc = self
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			if !doc.open_refcounts.contains_key(&sid) {
				return Err(ErrorCode::SyncDocNotFound);
			}
			if let Some(preferred) = doc.preferred_owner
				&& preferred != sid
			{
				return Err(ErrorCode::NotPreferredOwner);
			}
			if doc.owner != Some(sid) {
				return Err(ErrorCode::NotPreferredOwner);
			}
			if doc.owner_needs_resync {
				return Err(ErrorCode::OwnerNeedsResync);
			}

			let Some(history) = &self.history else {
				return Err(ErrorCode::Internal);
			};
			let Some(meta) = doc.history.as_ref() else {
				return Err(ErrorCode::Internal);
			};
			let head_id = meta.head_id;

			let Some((child_id, redo_wire)) = history.load_redo(&uri, head_id).map_err(|err| {
				tracing::warn!(error = %err, ?uri, "history redo load failed");
				ErrorCode::Internal
			})?
			else {
				return Err(ErrorCode::NothingToRedo);
			};

			let tx = wire_convert::wire_to_tx(&redo_wire, doc.rope.slice(..))
				.map_err(|_| ErrorCode::InvalidDelta)?;
			tx.apply(&mut doc.rope);
			doc.seq = SyncSeq(doc.seq.0 + 1);
			doc.update_fingerprint();
			doc.touch(sid);

			{
				let Some(meta) = doc.history.as_mut() else {
					return Err(ErrorCode::Internal);
				};
				meta.head_id = child_id;
				history
					.update_doc_state(&uri, meta, doc.epoch, doc.seq, doc.hash64, doc.len_chars)
					.map_err(|err| {
						tracing::warn!(error = %err, ?uri, "history redo persist failed");
						ErrorCode::Internal
					})?;
			}

			let event = Event::SharedDelta {
				uri: uri.clone(),
				epoch: doc.epoch,
				seq: doc.seq,
				tx: redo_wire,
			};
			let lsp_text = doc.rope.to_string();
			let participants = doc.participants.clone();
			let seq = doc.seq;
			let final_epoch = doc.epoch;
			(event, lsp_text, participants, seq, final_epoch)
		};

		self.sessions
			.broadcast(
				participants,
				xeno_broker_proto::types::IpcFrame::Event(event),
				Some(sid),
			)
			.await;

		self.notify_lsp_update(uri.clone(), lsp_text).await;

		if let Some(knowledge) = &self.knowledge {
			let _ = knowledge.doc_dirty(uri);
		}

		Ok(ResponsePayload::SharedRedoAck {
			epoch: final_epoch,
			seq,
		})
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
		Ok(ResponsePayload::SharedActivityAck)
	}

	async fn handle_idle_tick(&mut self) {
		let now = Instant::now();
		let mut unlocks = Vec::new();
		let history = self.history.as_ref();

		for (uri, doc) in &mut self.sync_docs {
			if doc.owner_idle(now) {
				unlocks.push(Self::prepare_unlock(history, uri, doc));
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

	fn persist_doc_state(history: Option<&HistoryStore>, uri: &str, doc: &SyncDocState) {
		let Some(history) = history else {
			return;
		};
		let Some(meta) = doc.history.as_ref() else {
			return;
		};
		if let Err(err) =
			history.update_doc_state(uri, meta, doc.epoch, doc.seq, doc.hash64, doc.len_chars)
		{
			tracing::warn!(error = %err, ?uri, "history metadata update failed");
		}
	}

	fn prepare_unlock(
		history: Option<&HistoryStore>,
		uri: &str,
		doc: &mut SyncDocState,
	) -> (Vec<SessionId>, Event) {
		doc.owner = None;
		doc.epoch = SyncEpoch(doc.epoch.0 + 1);
		doc.seq = SyncSeq(0);
		doc.owner_needs_resync = true;

		Self::persist_doc_state(history, uri, doc);

		let snapshot = doc.snapshot(uri);
		let event = Event::SharedUnlocked { snapshot };
		(doc.participants.clone(), event)
	}

	async fn handle_focus(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		focused: bool,
		focus_seq: u64,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let history = self.history.as_ref();
		let doc = self
			.sync_docs
			.get_mut(&uri)
			.ok_or(ErrorCode::SyncDocNotFound)?;

		if !doc.open_refcounts.contains_key(&sid) {
			return Err(ErrorCode::SyncDocNotFound);
		}

		// focus_seq ordering guard
		let last_seq = doc.last_focus_seq.get(&sid).copied().unwrap_or(0);
		if focus_seq <= last_seq && last_seq != 0 {
			return Ok(ResponsePayload::SharedFocusAck {
				snapshot: doc.snapshot(&uri),
			});
		}
		doc.last_focus_seq.insert(sid, focus_seq);

		let mut preferred_changed = false;
		let mut owner_changed = None;

		if focused {
			if doc.preferred_owner != Some(sid) {
				doc.preferred_owner = Some(sid);
				preferred_changed = true;
			}
			if doc.owner != Some(sid) {
				doc.owner = Some(sid);
				doc.epoch = SyncEpoch(doc.epoch.0 + 1);
				doc.seq = SyncSeq(0);
				doc.owner_needs_resync = true;
				owner_changed = Some(true);
			}
			doc.touch(sid);
		} else {
			if doc.preferred_owner == Some(sid) {
				doc.preferred_owner = None;
				preferred_changed = true;
			}
			if doc.owner == Some(sid) && doc.preferred_owner != Some(sid) {
				doc.owner = None;
				doc.epoch = SyncEpoch(doc.epoch.0 + 1);
				doc.seq = SyncSeq(0);
				doc.owner_needs_resync = true;
				owner_changed = Some(false);
			}
		}

		if owner_changed.is_some() {
			Self::persist_doc_state(history, &uri, doc);
		}

		let snapshot = doc.snapshot(&uri);
		let participants = doc.participants.clone();
		if preferred_changed {
			let event = Event::SharedPreferredOwnerChanged {
				snapshot: snapshot.clone(),
			};
			self.sessions
				.broadcast(
					participants.clone(),
					xeno_broker_proto::types::IpcFrame::Event(event),
					None,
				)
				.await;
		}
		if let Some(owner_set) = owner_changed {
			let event = if owner_set {
				Event::SharedOwnerChanged {
					snapshot: snapshot.clone(),
				}
			} else {
				Event::SharedUnlocked {
					snapshot: snapshot.clone(),
				}
			};
			self.sessions
				.broadcast(
					participants,
					xeno_broker_proto::types::IpcFrame::Event(event),
					None,
				)
				.await;
		}

		Ok(ResponsePayload::SharedFocusAck { snapshot })
	}

	async fn handle_resync(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		client_hash64: Option<u64>,
		client_len_chars: Option<u64>,
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

		let snapshot = doc.snapshot(&uri);

		// Conditional resync
		if let (Some(h), Some(l)) = (client_hash64, client_len_chars)
			&& h == doc.hash64
			&& l == doc.len_chars
		{
			return Ok(ResponsePayload::SharedSnapshot {
				text: String::new(),
				snapshot,
			});
		}

		Ok(ResponsePayload::SharedSnapshot {
			text: doc.rope.to_string(),
			snapshot,
		})
	}

	async fn handle_session_cleanup(&mut self, sid: SessionId) {
		let uris: Vec<String> = self
			.sync_docs
			.iter()
			.filter(|(_, doc)| doc.open_refcounts.contains_key(&sid))
			.map(|(uri, _)| uri.clone())
			.collect();

		let mut closed_uris = Vec::new();
		let history = self.history.as_ref();
		for uri in uris {
			let mut events: Vec<(Vec<SessionId>, Event)> = Vec::new();
			if let Some(doc) = self.sync_docs.get_mut(&uri) {
				let mut preferred_changed = false;
				if doc.preferred_owner == Some(sid) {
					doc.preferred_owner = None;
					preferred_changed = true;
				}

				doc.remove_participant_all(sid);
				if doc.participants.is_empty() {
					closed_uris.push(uri.clone());
				} else {
					if preferred_changed {
						events.push((
							doc.participants.clone(),
							Event::SharedPreferredOwnerChanged {
								snapshot: doc.snapshot(&uri),
							},
						));
					}
					if doc.owner == Some(sid) {
						events.push(Self::prepare_unlock(history, &uri, doc));
					}
				}
			}
			for (targets, event) in events {
				self.sessions
					.broadcast(
						targets,
						xeno_broker_proto::types::IpcFrame::Event(event),
						None,
					)
					.await;
			}
		}
		for uri in closed_uris {
			self.sync_docs.remove(&uri);
			self.open_docs_set.lock().unwrap().remove(&uri);
			self.notify_lsp_close(uri).await;
		}
	}

	async fn notify_lsp_open(&self, uri: String, text: String) {
		let Some(routing) = self.routing.clone() else {
			return;
		};
		routing.lsp_doc_open(uri, text).await;
	}

	async fn notify_lsp_update(&self, uri: String, text: String) {
		let Some(routing) = self.routing.clone() else {
			return;
		};
		routing.lsp_doc_update(uri, text).await;
	}

	async fn notify_lsp_close(&self, uri: String) {
		let Some(routing) = self.routing.clone() else {
			return;
		};
		routing.lsp_doc_close(uri).await;
	}
}
