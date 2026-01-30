//! Shared broker core state.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use xeno_broker_proto::types::{DocId, IpcFrame, Request, Response, ServerId, SessionId};
use xeno_rpc::{MainLoopEvent, PeerSocket};

/// Sink for sending events to a connected session.
pub type SessionSink = PeerSocket<IpcFrame, Request, Response>;

/// Sink for sending messages to an LSP server.
pub type LspTx = PeerSocket<xeno_lsp::Message, xeno_lsp::AnyRequest, xeno_lsp::AnyResponse>;

/// Result of a server-to-editor request.
pub type LspReplyResult = Result<serde_json::Value, xeno_lsp::ResponseError>;

/// Shared state for the broker.
///
/// This registry tracks active editor sessions, running LSP server instances,
/// and document version state to enable asynchronous proxying and event fan-out.
#[derive(Debug, Default)]
pub struct BrokerCore {
	/// Connected sessions and their event sinks.
	sessions: Mutex<HashMap<SessionId, SessionSink>>,
	/// Running LSP server instances.
	servers: Mutex<HashMap<ServerId, LspInstance>>,
	/// Pending requests initiated by LSP servers awaiting editor replies.
	pending_client_reqs:
		Mutex<HashMap<(ServerId, xeno_lsp::RequestId), oneshot::Sender<LspReplyResult>>>,
	/// Document registry for version tracking.
	docs: Mutex<DocRegistry>,
	/// Next available server ID.
	next_server_id: AtomicU64,
}

impl BrokerCore {
	/// Create a new broker core instance.
	#[must_use]
	pub fn new() -> Arc<Self> {
		Arc::new(Self::default())
	}

	/// Register an editor session with its outbound event sink.
	pub fn register_session(&self, session_id: SessionId, sink: SessionSink) {
		self.sessions.lock().unwrap().insert(session_id, sink);
	}

	/// Unregister an editor session.
	pub fn unregister_session(&self, session_id: SessionId) {
		self.sessions.lock().unwrap().remove(&session_id);
	}

	/// Send an asynchronous event to a registered session.
	///
	/// If the session is no longer connected, the event is silently dropped.
	pub fn send_event(&self, session_id: SessionId, event: IpcFrame) {
		let sessions = self.sessions.lock().unwrap();
		if let Some(sink) = sessions.get(&session_id) {
			let _ = sink.send(MainLoopEvent::Outgoing(event));
		}
	}

	/// Broadcast an event to all sessions.
	pub fn broadcast_event(&self, event: IpcFrame) {
		let sessions = self.sessions.lock().unwrap();
		for sink in sessions.values() {
			let _ = sink.send(MainLoopEvent::Outgoing(event.clone()));
		}
	}

	/// Register a new running LSP instance.
	pub fn register_server(&self, server_id: ServerId, instance: LspInstance) {
		self.servers.lock().unwrap().insert(server_id, instance);
	}

	/// Unregister an LSP instance and release its resources.
	pub fn unregister_server(&self, server_id: ServerId) {
		self.servers.lock().unwrap().remove(&server_id);
	}

	/// Retrieves the communication handle for a specific LSP server.
	pub fn get_server_tx(&self, server_id: ServerId) -> Option<LspTx> {
		self.servers
			.lock()
			.unwrap()
			.get(&server_id)
			.map(|s| s.lsp_tx.clone())
	}

	/// Register a pending server-to-editor request.
	pub fn register_client_request(
		&self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		tx: oneshot::Sender<LspReplyResult>,
	) {
		self.pending_client_reqs
			.lock()
			.unwrap()
			.insert((server_id, request_id), tx);
	}

	/// Complete a pending server-to-editor request with a reply.
	pub fn complete_client_request(
		&self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		result: LspReplyResult,
	) -> bool {
		if let Some(tx) = self
			.pending_client_reqs
			.lock()
			.unwrap()
			.remove(&(server_id, request_id))
		{
			let _ = tx.send(result);
			true
		} else {
			false
		}
	}

	/// Update server status and notify the owning session.
	///
	/// Only emits an event if the status has actually changed.
	pub fn set_server_status(
		&self,
		server_id: ServerId,
		status: xeno_broker_proto::types::LspServerStatus,
	) {
		let (owner, changed) = {
			let servers = self.servers.lock().unwrap();
			if let Some(instance) = servers.get(&server_id) {
				let mut current = instance.status.lock().unwrap();
				if *current != status {
					*current = status;
					(Some(instance.owner), true)
				} else {
					(Some(instance.owner), false)
				}
			} else {
				(None, false)
			}
		};

		if changed && let Some(owner) = owner {
			self.send_event(
				owner,
				IpcFrame::Event(xeno_broker_proto::types::Event::LspStatus { server_id, status }),
			);
		}
	}

	/// Allocate a globally unique server ID.
	pub fn next_server_id(&self) -> ServerId {
		ServerId(self.next_server_id.fetch_add(1, Ordering::Relaxed))
	}

	/// Retrieve the DocId and last known version for a URI.
	pub fn get_doc_by_uri(&self, uri: &str) -> Option<(DocId, u32)> {
		self.docs.lock().unwrap().by_uri.get(uri).cloned()
	}

	/// Update document version tracking by observing editor-to-server traffic.
	///
	/// Currently monitors `didOpen` and `didChange` notifications.
	pub fn on_editor_message(&self, _server_id: ServerId, msg: &xeno_lsp::Message) {
		if let xeno_lsp::Message::Notification(notif) = msg {
			let mut docs = self.docs.lock().unwrap();
			match notif.method.as_str() {
				"textDocument/didOpen" | "textDocument/didChange" => {
					if let Some(doc) = notif.params.get("textDocument")
						&& let Some(uri) = doc.get("uri").and_then(|u| u.as_str())
					{
						let version =
							doc.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
						docs.update(uri.to_string(), version);
					}
				}
				_ => {}
			}
		}
	}
}

/// A running LSP server instance and its associated handles.
#[derive(Debug)]
pub struct LspInstance {
	/// The session that started this server.
	pub owner: SessionId,
	/// Globally unique server identifier.
	pub server_id: ServerId,
	/// Socket for sending requests/notifications to the server's stdio.
	pub lsp_tx: LspTx,
	/// Child process handle for lifecycle management.
	pub child: tokio::process::Child,
	/// Synchronized server lifecycle status.
	pub status: Mutex<xeno_broker_proto::types::LspServerStatus>,
}

/// Registry for document version tracking.
///
/// Maps document URIs to internal DocIds and the latest version reported
/// by the editor. This enables the broker to correlate diagnostics with
/// specific document states.
#[derive(Debug, Default)]
struct DocRegistry {
	/// Map of URI to (DocId, last_version).
	by_uri: HashMap<String, (DocId, u32)>,
	/// Next available DocId.
	next_doc_id: u64,
}

impl DocRegistry {
	/// Update or create document version information.
	fn update(&mut self, uri: String, version: u32) {
		if let Some(info) = self.by_uri.get_mut(&uri) {
			info.1 = version;
		} else {
			let id = DocId(self.next_doc_id);
			self.next_doc_id += 1;
			self.by_uri.insert(uri, (id, version));
		}
	}
}
