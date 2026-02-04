use ropey::Rope;
use tokio::sync::{mpsc, oneshot};
use xeno_broker_proto::types::{
	ErrorCode, ResponsePayload, SessionId, SharedApplyKind, SyncEpoch, SyncNonce, SyncSeq, WireTx,
};

use super::commands::SharedStateCmd;

/// Handle for communicating with the [`SharedStateService`] actor.
#[derive(Clone, Debug)]
pub struct SharedStateHandle {
	tx: mpsc::Sender<SharedStateCmd>,
}

impl SharedStateHandle {
	/// Wraps a command sender in a typed handle.
	pub fn new(tx: mpsc::Sender<SharedStateCmd>) -> Self {
		Self { tx }
	}

	/// Opens a document or joins an existing session.
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

	/// Closes a document for a session.
	pub async fn close(&self, sid: SessionId, uri: String) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Close { sid, uri, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Apply a shared mutation (Edit/Undo/Redo).
	#[allow(clippy::too_many_arguments)]
	pub async fn apply(
		&self,
		sid: SessionId,
		uri: String,
		kind: SharedApplyKind,
		epoch: SyncEpoch,
		base_seq: SyncSeq,
		base_hash64: u64,
		base_len_chars: u64,
		tx: Option<WireTx>,
		undo_group: u64,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Apply {
				sid,
				uri,
				kind,
				epoch,
				base_seq,
				base_hash64,
				base_len_chars,
				tx,
				undo_group,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Updates activity timestamp for a document.
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

	/// Updates focus status for a document with atomic ownership acquisition.
	pub async fn focus(
		&self,
		sid: SessionId,
		uri: String,
		focused: bool,
		focus_seq: u64,
		nonce: SyncNonce,
		client_hash64: Option<u64>,
		client_len_chars: Option<u64>,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Focus {
				sid,
				uri,
				focused,
				focus_seq,
				nonce,
				client_hash64,
				client_len_chars,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Fetch a full snapshot of the authoritative document.
	pub async fn resync(
		&self,
		sid: SessionId,
		uri: String,
		nonce: SyncNonce,
		client_hash64: Option<u64>,
		client_len_chars: Option<u64>,
	) -> Result<ResponsePayload, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(SharedStateCmd::Resync {
				sid,
				uri,
				nonce,
				client_hash64,
				client_len_chars,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Authoritatively cleans up a lost session.
	pub async fn session_lost(&self, sid: SessionId) {
		let _ = self.tx.send(SharedStateCmd::SessionLost { sid }).await;
	}

	/// Returns a triad of epoch, seq, and rope for a document.
	pub async fn snapshot(&self, uri: String) -> Option<(SyncEpoch, SyncSeq, Rope)> {
		let (reply, rx) = oneshot::channel();
		let _ = self.tx.send(SharedStateCmd::Snapshot { uri, reply }).await;
		rx.await.ok().flatten()
	}

	/// Returns true if the document is currently open in the broker.
	pub async fn is_open(&self, uri: String) -> bool {
		let (reply, rx) = oneshot::channel();
		let _ = self.tx.send(SharedStateCmd::IsOpen { uri, reply }).await;
		rx.await.unwrap_or(false)
	}
}
