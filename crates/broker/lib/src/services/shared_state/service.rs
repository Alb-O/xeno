use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use helix_db::helix_engine::storage_core::HelixGraphStorage;
use ropey::Rope;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, interval};
use xeno_broker_proto::types::{
	DocStateSnapshot, DocSyncPhase, ErrorCode, Event, ResponsePayload, SessionId, SharedApplyKind,
	SyncEpoch, SyncNonce, SyncSeq, WireTx,
};

use super::commands::SharedStateCmd;
use super::handle::SharedStateHandle;
use crate::core::history::{HistoryMeta, HistoryStore};
use crate::services::{knowledge, routing, sessions};
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
	/// Captures a static snapshot of the current document state.
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
			history_head_id: self.history.as_ref().map(|h| h.head_id),
			history_root_id: self.history.as_ref().map(|h| h.root_id),
			history_head_group: self.history.as_ref().map(|h| h.head_group_id),
		}
	}

	/// Recalculates the fingerprint (hash and length) from the current rope.
	fn update_fingerprint(&mut self) {
		let (len, hash) = xeno_broker_proto::fingerprint_rope(&self.rope);
		self.len_chars = len;
		self.hash64 = hash;
	}

	/// Registers a session as an active participant for this document.
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

	/// Deregisters a session or decrements its reference count.
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

	/// Forcibly removes a session from all participation tracking.
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

	/// Updates the activity timestamp for a session.
	fn touch(&mut self, sid: SessionId) {
		self.last_active.insert(sid, Instant::now());
	}

	/// Returns true if the current owner has exceeded the idle timeout.
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

/// Actor service managing authoritative document consistency and synchronization.
pub struct SharedStateService {
	rx: mpsc::Receiver<SharedStateCmd>,
	sync_docs: HashMap<String, SyncDocState>,
	history: Option<HistoryStore>,
	/// Shared set of open URIs exposed to the knowledge crawler.
	open_docs_set: Arc<Mutex<HashSet<String>>>,
	sessions: sessions::SessionHandle,
	knowledge: Option<knowledge::KnowledgeHandle>,
	routing: Option<routing::RoutingHandle>,
}

impl SharedStateService {
	/// Spawns the shared state service actor.
	pub fn start(
		sessions: sessions::SessionHandle,
		storage: Option<Arc<HelixGraphStorage>>,
	) -> (
		SharedStateHandle,
		Arc<Mutex<HashSet<String>>>,
		mpsc::Sender<knowledge::KnowledgeHandle>,
		mpsc::Sender<routing::RoutingHandle>,
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
		mut knowledge_rx: mpsc::Receiver<knowledge::KnowledgeHandle>,
		mut routing_rx: mpsc::Receiver<routing::RoutingHandle>,
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
						SharedStateCmd::Open { sid, uri, text, version_hint: _, reply } => {
							let _ = reply.send(self.handle_open(sid, &uri, &text).await);
						}
						SharedStateCmd::Close { sid, uri, reply } => {
							let _ = reply.send(self.handle_close(sid, &uri).await);
						}
						SharedStateCmd::Apply { sid, uri, kind, epoch, base_seq, base_hash64, base_len_chars, tx, undo_group, reply } => {
							let _ = reply.send(self.handle_apply(sid, &uri, kind, epoch, base_seq, base_hash64, base_len_chars, tx.as_ref(), undo_group).await);
						}
						SharedStateCmd::Activity { sid, uri, reply } => {
							let _ = reply.send(self.handle_activity(sid, &uri));
						}
						SharedStateCmd::Focus { sid, uri, focused, focus_seq, nonce, client_hash64, client_len_chars, reply } => {
							let _ = reply.send(self.handle_focus(sid, &uri, focused, focus_seq, nonce, client_hash64, client_len_chars).await);
						}
						SharedStateCmd::Resync { sid, uri, nonce, client_hash64, client_len_chars, reply } => {
							let _ = reply.send(self.handle_resync(sid, &uri, nonce, client_hash64, client_len_chars).await);
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

	/// Handles a request to join document synchronization.
	///
	/// If the document is not already open, it is loaded from history or
	/// created with the provided initial text.
	async fn handle_open(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		text: &str,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;
		let mut routing_open = None;

		let (snapshot, res_text) = match self.sync_docs.get_mut(&uri) {
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
					let init_rope = Rope::from(text);
					let (init_len, init_hash) = xeno_broker_proto::fingerprint_rope(&init_rope);

					match history.load_or_create_doc(
						&uri, &init_rope, doc.epoch, doc.seq, init_hash, init_len,
					) {
						Ok((stored, created)) => {
							doc.epoch = stored.epoch;
							doc.seq = stored.seq;
							doc.rope = stored.rope;
							doc.hash64 = stored.hash64;
							doc.len_chars = stored.len_chars;
							doc.history = Some(stored.meta);

							let (calc_len, calc_hash) =
								xeno_broker_proto::fingerprint_rope(&doc.rope);
							if doc.len_chars != calc_len || doc.hash64 != calc_hash {
								tracing::warn!(
									?uri,
									"history fingerprint mismatch; repairing metadata"
								);
								doc.len_chars = calc_len;
								doc.hash64 = calc_hash;
								let _ = history.update_doc_state(
									&uri,
									doc.history.as_ref().unwrap(),
									doc.epoch,
									doc.seq,
									doc.hash64,
									doc.len_chars,
								);
							}

							if !created {
								send_text = Some(doc.rope.to_string());
							}
						}
						Err(err) => {
							tracing::warn!(error = %err, ?uri, "history load-or-create failed; continuing without history");
							doc.rope = Rope::from(text);
							doc.update_fingerprint();
							doc.history = None;
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

		Ok(ResponsePayload::SharedOpened {
			snapshot,
			text: res_text,
		})
	}

	/// Handles a request to depart from document synchronization.
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
			self.notify_lsp_close(uri).await;
		} else if let Some((targets, event)) = unlock {
			self.sessions
				.broadcast(
					targets,
					xeno_broker_proto::types::IpcFrame::Event(event),
					None,
				)
				.await;
		}

		Ok(ResponsePayload::SharedClosed)
	}

	/// Applies an authoritative mutation to the document state.
	///
	/// Validates ownership and preconditions before applying the delta.
	/// Broadcasts the delta to all participants.
	#[allow(clippy::too_many_arguments)]
	async fn handle_apply(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		kind: SharedApplyKind,
		epoch: SyncEpoch,
		base_seq: SyncSeq,
		base_hash64: u64,
		base_len_chars: u64,
		wire_tx: Option<&WireTx>,
		undo_group: u64,
	) -> Result<ResponsePayload, ErrorCode> {
		let uri = crate::core::normalize_uri(uri_in)?;

		let (event, participants, ack) = {
			let doc = self
				.sync_docs
				.get_mut(&uri)
				.ok_or(ErrorCode::SyncDocNotFound)?;

			if !doc.open_refcounts.contains_key(&sid) {
				return Err(ErrorCode::SyncDocNotFound);
			}
			if doc.preferred_owner.is_some_and(|p| p != sid) {
				return Err(ErrorCode::NotPreferredOwner);
			}
			if doc.owner != Some(sid) {
				return Err(ErrorCode::NotPreferredOwner);
			}
			if epoch != doc.epoch {
				return Err(ErrorCode::SyncEpochMismatch);
			}
			if doc.owner_needs_resync {
				return Err(ErrorCode::OwnerNeedsResync);
			}

			if base_seq != doc.seq {
				doc.owner_needs_resync = true;
				return Err(ErrorCode::SyncSeqMismatch);
			}
			if base_hash64 != doc.hash64 || base_len_chars != doc.len_chars {
				doc.owner_needs_resync = true;
				return Err(ErrorCode::SyncFingerprintMismatch);
			}

			let mut applied_wire: Option<WireTx> = None;
			let history_from_id = doc.history.as_ref().map(|h| h.head_id);
			let mut history_group = None;

			match kind {
				SharedApplyKind::Edit => {
					let wire_tx = wire_tx.ok_or(ErrorCode::InvalidArgs)?;

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
					doc.seq = SyncSeq(doc.seq.0.wrapping_add(1));
					doc.update_fingerprint();
					doc.touch(sid);
					history_group = Some(undo_group);

					if let Some(history) = &self.history
						&& let Some(meta) = doc.history.as_mut()
					{
						let undo_wire = wire_convert::tx_to_wire(&undo_tx);
						if let Err(err) = history.append_edit_with_checkpoint(
							&uri,
							meta,
							doc.epoch,
							doc.seq,
							doc.hash64,
							doc.len_chars,
							undo_group,
							sid.0,
							wire_tx.clone(),
							undo_wire,
							MAX_HISTORY_NODES,
						) {
							tracing::warn!(error = %err, ?uri, "history append failed; disabling history for doc");
							doc.history = None;
						}
					}
				}

				SharedApplyKind::Undo => {
					let history = self.history.as_ref().ok_or(ErrorCode::NotImplemented)?;
					let meta = doc.history.as_ref().ok_or(ErrorCode::HistoryUnavailable)?;

					let (new_head, new_group, undo_wire) = history
						.load_undo_group(&uri, meta, &doc.rope)
						.map_err(|err| {
							tracing::warn!(error = %err, ?uri, "history undo load failed");
							ErrorCode::Internal
						})?
						.ok_or(ErrorCode::NothingToUndo)?;

					let tx = wire_convert::wire_to_tx(&undo_wire, doc.rope.slice(..))
						.map_err(|_| ErrorCode::InvalidDelta)?;
					tx.apply(&mut doc.rope);

					doc.seq = SyncSeq(doc.seq.0.wrapping_add(1));
					doc.update_fingerprint();
					doc.touch(sid);
					history_group = Some(doc.history.as_ref().unwrap().head_group_id);

					let meta_mut = doc.history.as_mut().unwrap();
					meta_mut.head_id = new_head;
					meta_mut.head_group_id = new_group;

					if let Err(err) = history.update_doc_state(
						&uri,
						meta_mut,
						doc.epoch,
						doc.seq,
						doc.hash64,
						doc.len_chars,
					) {
						tracing::warn!(error = %err, ?uri, "history undo persist failed; disabling history for doc");
						doc.history = None;
					}

					applied_wire = Some(undo_wire);
				}

				SharedApplyKind::Redo => {
					let history = self.history.as_ref().ok_or(ErrorCode::NotImplemented)?;
					let meta = doc.history.as_ref().ok_or(ErrorCode::HistoryUnavailable)?;

					let (new_head, new_group, redo_wire) = history
						.load_redo_group(&uri, meta, &doc.rope)
						.map_err(|err| {
							tracing::warn!(error = %err, ?uri, "history redo load failed");
							ErrorCode::Internal
						})?
						.ok_or(ErrorCode::NothingToRedo)?;

					let tx = wire_convert::wire_to_tx(&redo_wire, doc.rope.slice(..))
						.map_err(|_| ErrorCode::InvalidDelta)?;
					tx.apply(&mut doc.rope);

					doc.seq = SyncSeq(doc.seq.0.wrapping_add(1));
					doc.update_fingerprint();
					doc.touch(sid);
					history_group = Some(new_group);

					let meta_mut = doc.history.as_mut().unwrap();
					meta_mut.head_id = new_head;
					meta_mut.head_group_id = new_group;

					if let Err(err) = history.update_doc_state(
						&uri,
						meta_mut,
						doc.epoch,
						doc.seq,
						doc.hash64,
						doc.len_chars,
					) {
						tracing::warn!(error = %err, ?uri, "history redo persist failed; disabling history for doc");
						doc.history = None;
					}

					applied_wire = Some(redo_wire);
				}
			}

			let tx_for_broadcast = applied_wire
				.as_ref()
				.cloned()
				.unwrap_or_else(|| wire_tx.unwrap().clone());

			let history_to_id = doc.history.as_ref().map(|h| h.head_id);

			let event = xeno_broker_proto::types::Event::SharedDelta {
				uri: uri.clone(),
				epoch: doc.epoch,
				seq: doc.seq,
				kind,
				tx: tx_for_broadcast,
				origin: sid,
				hash64: doc.hash64,
				len_chars: doc.len_chars,
				history_from_id,
				history_to_id,
				history_group,
			};

			let ack = ResponsePayload::SharedApplyAck {
				uri: uri.clone(),
				kind,
				epoch: doc.epoch,
				seq: doc.seq,
				applied_tx: applied_wire,
				hash64: doc.hash64,
				len_chars: doc.len_chars,
				history_from_id,
				history_to_id,
				history_group,
			};

			(event, doc.participants.clone(), ack)
		};

		self.sessions
			.broadcast(
				participants,
				xeno_broker_proto::types::IpcFrame::Event(event),
				Some(sid),
			)
			.await;

		if let Some(doc) = self.sync_docs.get(&uri) {
			self.notify_lsp_update(uri.clone(), doc.rope.to_string())
				.await;
		}

		if let Some(knowledge) = &self.knowledge {
			let _ = knowledge.doc_dirty(uri);
		}

		Ok(ack)
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
		if let (Some(history), Some(meta)) = (history, doc.history.as_ref())
			&& let Err(err) =
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
		nonce: SyncNonce,
		client_hash64: Option<u64>,
		client_len_chars: Option<u64>,
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

		let last_seq = doc.last_focus_seq.get(&sid).copied().unwrap_or(0);
		if focus_seq <= last_seq && last_seq != 0 {
			return Ok(ResponsePayload::SharedFocusAck {
				nonce,
				snapshot: doc.snapshot(&uri),
				repair_text: None,
			});
		}
		doc.last_focus_seq.insert(sid, focus_seq);

		let mut preferred_changed = false;
		let mut owner_changed = None;
		let mut repair_text: Option<String> = None;

		if focused {
			if doc.preferred_owner != Some(sid) {
				doc.preferred_owner = Some(sid);
				preferred_changed = true;
			}

			if doc.owner != Some(sid) {
				doc.owner = Some(sid);
				doc.epoch = SyncEpoch(doc.epoch.0 + 1);
				doc.seq = SyncSeq(0);
				owner_changed = Some(true);
			}

			if let (Some(h), Some(l)) = (client_hash64, client_len_chars) {
				if h != doc.hash64 || l != doc.len_chars {
					repair_text = Some(doc.rope.to_string());
				}
			} else {
				repair_text = Some(doc.rope.to_string());
			}

			doc.owner_needs_resync = false;
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

		Ok(ResponsePayload::SharedFocusAck {
			nonce,
			snapshot,
			repair_text,
		})
	}

	async fn handle_resync(
		&mut self,
		sid: SessionId,
		uri_in: &str,
		nonce: SyncNonce,
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

		if let (Some(h), Some(l)) = (client_hash64, client_len_chars)
			&& h == doc.hash64
			&& l == doc.len_chars
		{
			return Ok(ResponsePayload::SharedSnapshot {
				nonce,
				text: String::new(),
				snapshot,
			});
		}

		Ok(ResponsePayload::SharedSnapshot {
			nonce,
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
		if let Some(routing) = &self.routing {
			routing.lsp_doc_open(uri, text).await;
		}
	}

	async fn notify_lsp_update(&self, uri: String, text: String) {
		if let Some(routing) = &self.routing {
			routing.lsp_doc_update(uri, text).await;
		}
	}

	async fn notify_lsp_close(&self, uri: String) {
		if let Some(routing) = &self.routing {
			routing.lsp_doc_close(uri).await;
		}
	}
}
