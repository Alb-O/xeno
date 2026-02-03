//! LSP routing and lifecycle management service.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use xeno_broker_proto::types::{
	ErrorCode, Event, LspServerConfig, LspServerStatus, ServerId, SessionId,
};
use xeno_lsp::{AnyNotification, AnyRequest, AnyResponse, Message, RequestId};
use xeno_rpc::MainLoopEvent;

use crate::core::text_sync::{DocGateDecision, DocOwnerState};
use crate::launcher::LspLauncher;
use crate::services::knowledge::KnowledgeHandle;
use crate::services::sessions::SessionHandle;

/// Metadata for a managed LSP server instance.
pub struct ServerEntry {
	/// Communication handle and process monitor.
	pub instance: crate::core::LspInstance,
	/// Identity of the project (command/args/cwd).
	pub project: crate::core::ProjectKey,
	/// Set of editor sessions currently participating.
	pub attached: HashSet<SessionId>,
	/// Session ID elected to handle server-to-client requests.
	pub leader: SessionId,
	/// Tracker for document versions on this server.
	pub docs: crate::core::text_sync::DocRegistry,
	/// Broker-owned LSP document state keyed by URI.
	lsp_docs: HashMap<String, LspDocState>,
	/// Token for invalidating stale lease timers.
	pub lease_gen: u64,
	/// Ownership tracker for text sync gating.
	pub doc_owners: crate::core::text_sync::DocOwnerRegistry,
	/// Monotonic sequence for broker-originated request IDs.
	pub next_wire_req_id: u64,
}

#[derive(Debug, Clone)]
struct LspDocState {
	language_id: Option<String>,
	text: String,
	version: u32,
	open: bool,
}

#[derive(Debug)]
enum LspDocAction {
	Open {
		uri: String,
		language_id: String,
		version: u32,
		text: String,
	},
	Change {
		uri: String,
		version: u32,
		text: String,
	},
}

#[derive(Debug, Clone)]
struct LspContentChange {
	range: Option<LspRange>,
	text: String,
}

#[derive(Debug, Clone)]
struct LspRange {
	start: LspPosition,
	end: LspPosition,
}

#[derive(Debug, Clone, Copy)]
struct LspPosition {
	line: u32,
	character: u32,
}

fn apply_content_changes(base: &str, changes: &[LspContentChange]) -> Option<String> {
	let mut text = base.to_string();
	if changes.is_empty() {
		return Some(text);
	}
	for change in changes {
		match &change.range {
			None => {
				text = change.text.clone();
			}
			Some(range) => {
				let start = lsp_offset(&text, range.start);
				let end = lsp_offset(&text, range.end);
				if start > end || start > text.len() || end > text.len() {
					return None;
				}
				text.replace_range(start..end, &change.text);
			}
		}
	}
	Some(text)
}

fn lsp_offset(text: &str, pos: LspPosition) -> usize {
	let mut line_start = 0usize;
	let mut current_line = 0u32;

	for (i, ch) in text.char_indices() {
		if current_line == pos.line {
			break;
		}
		if ch == '\n' {
			current_line += 1;
			line_start = i + ch.len_utf8();
		}
	}

	if current_line < pos.line {
		return text.len();
	}

	let line_slice = &text[line_start..];
	let line_end_rel = line_slice.find('\n').unwrap_or(line_slice.len());
	let line_end = line_start + line_end_rel;

	let mut utf16_units = 0u32;
	let mut byte = line_start;
	for (i, ch) in line_slice[..line_end_rel].char_indices() {
		let units = ch.len_utf16() as u32;
		if utf16_units + units > pos.character {
			break;
		}
		utf16_units += units;
		byte = line_start + i + ch.len_utf8();
		if utf16_units == pos.character {
			break;
		}
	}

	if utf16_units < pos.character {
		line_end
	} else {
		byte
	}
}

#[derive(Debug)]
struct PendingS2cReq {
	responder: SessionId,
	tx: oneshot::Sender<crate::core::LspReplyResult>,
}

#[derive(Debug)]
struct PendingC2sReq {
	origin_session: SessionId,
	origin_id: xeno_lsp::RequestId,
}

/// Commands for the routing service actor.
#[derive(Debug)]
pub enum RoutingCmd {
	/// Start or attach to an LSP server.
	LspStart {
		/// The session identity.
		sid: SessionId,
		/// The server process configuration.
		config: LspServerConfig,
		/// Reply channel for the server identity.
		reply: oneshot::Sender<Result<ServerId, ErrorCode>>,
	},
	/// Atomic operation to pin a leader, register a pending request, and transmit.
	BeginS2c {
		/// Target server.
		server_id: ServerId,
		/// ID assigned by the LSP server.
		request_id: xeno_lsp::RequestId,
		/// JSON payload.
		json: String,
		/// Channel for delivering the eventual editor reply.
		tx: oneshot::Sender<crate::core::LspReplyResult>,
		/// Immediate reply channel for transmission status.
		reply: oneshot::Sender<Result<(), xeno_lsp::ResponseError>>,
	},
	/// Complete a server-to-client request with a response.
	CompleteS2c {
		/// The replying session.
		sid: SessionId,
		/// Source server.
		server_id: ServerId,
		/// Original request ID.
		request_id: xeno_lsp::RequestId,
		/// The response payload or error.
		result: crate::core::LspReplyResult,
		/// Reply channel for status.
		reply: oneshot::Sender<bool>,
	},
	/// Cancel a pending server-to-client request.
	CancelS2c {
		/// Source server.
		server_id: ServerId,
		/// Original request ID.
		request_id: xeno_lsp::RequestId,
	},
	/// Initiate an editor-to-server request.
	BeginC2s {
		/// The originating session.
		sid: SessionId,
		/// Target server.
		server_id: ServerId,
		/// The LSP request payload.
		req: AnyRequest,
		/// Maximum time to wait for a reply.
		timeout: Duration,
		/// Reply channel for the server response.
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	},
	/// Internal result of a client-to-server request.
	C2sResp {
		/// Source server.
		server_id: ServerId,
		/// The response payload.
		resp: AnyResponse,
		/// Reply channel for the origin.
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	},
	/// Internal signal that a client-to-server request timed out.
	C2sTimeout {
		/// Source server.
		server_id: ServerId,
		/// The wire ID assigned by the broker.
		wire_id: xeno_lsp::RequestId,
		/// Reply channel for the origin.
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	},
	/// Internal signal that transmitting a request to the LSP server failed.
	C2sSendFailed {
		/// Source server.
		server_id: ServerId,
		/// The wire ID assigned by the broker.
		wire_id: xeno_lsp::RequestId,
		/// Reply channel for the origin.
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	},
	/// Authoritative signal that a session has disconnected.
	SessionLost {
		/// The lost session identity.
		sid: SessionId,
	},
	/// Signal that an LSP process has exited or crashed.
	ServerExited {
		/// The exited server.
		server_id: ServerId,
		/// Whether the process returned a non-zero exit code.
		crashed: bool,
	},
	/// Signal that an idle lease has expired.
	LeaseExpired {
		/// The idle server.
		server_id: ServerId,
		/// The era this lease was scheduled in.
		generation: u64,
	},
	/// Transmit a notification to an LSP server.
	LspSendNotif {
		/// Originating session.
		sid: SessionId,
		/// Target server.
		server_id: ServerId,
		/// JSON message content.
		message: String,
		/// Reply channel for confirmation.
		reply: oneshot::Sender<Result<(), ErrorCode>>,
	},
	/// Handle an inbound notification from an LSP server.
	ServerNotif {
		/// Source server.
		server_id: ServerId,
		/// JSON message content.
		message: String,
	},
	/// Update broker-owned LSP document state from buffer sync (initial open).
	LspDocOpen {
		/// Canonical document URI.
		uri: String,
		/// Full document text.
		text: String,
	},
	/// Update broker-owned LSP document state from buffer sync (content change).
	LspDocUpdate {
		/// Canonical document URI.
		uri: String,
		/// Full document text.
		text: String,
	},
	/// Close a broker-owned LSP document (no active sessions).
	LspDocClose {
		/// Canonical document URI.
		uri: String,
	},
	/// Terminate all managed processes and shutdown the service.
	TerminateAll,
}

/// Handle for communicating with the `RoutingService`.
#[derive(Clone, Debug)]
pub struct RoutingHandle {
	tx: mpsc::Sender<RoutingCmd>,
}

impl RoutingHandle {
	/// Wraps a command sender in a typed handle.
	pub fn new(tx: mpsc::Sender<RoutingCmd>) -> Self {
		Self { tx }
	}

	/// Starts an LSP server.
	pub async fn lsp_start(
		&self,
		sid: SessionId,
		config: LspServerConfig,
	) -> Result<ServerId, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(RoutingCmd::LspStart { sid, config, reply })
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Registers and transmits a server-to-client request atomically.
	pub async fn begin_s2c(
		&self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		json: String,
		tx: oneshot::Sender<crate::core::LspReplyResult>,
	) -> Result<(), xeno_lsp::ResponseError> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(RoutingCmd::BeginS2c {
				server_id,
				request_id,
				json,
				tx,
				reply,
			})
			.await
			.map_err(|_| {
				xeno_lsp::ResponseError::new(
					xeno_lsp::ErrorCode::INTERNAL_ERROR,
					"broker shut down",
				)
			})?;
		rx.await.map_err(|_| {
			xeno_lsp::ResponseError::new(xeno_lsp::ErrorCode::INTERNAL_ERROR, "broker shut down")
		})?
	}

	/// Delivers a reply to a pending server-to-client request.
	pub async fn complete_s2c(
		&self,
		sid: SessionId,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		result: crate::core::LspReplyResult,
	) -> bool {
		let (reply, rx) = oneshot::channel();
		if self
			.tx
			.send(RoutingCmd::CompleteS2c {
				sid,
				server_id,
				request_id,
				result,
				reply,
			})
			.await
			.is_err()
		{
			return false;
		}
		rx.await.unwrap_or(false)
	}

	/// Cancels a server-to-client request.
	pub async fn cancel_s2c(&self, server_id: ServerId, request_id: xeno_lsp::RequestId) {
		let _ = self
			.tx
			.send(RoutingCmd::CancelS2c {
				server_id,
				request_id,
			})
			.await;
	}

	/// Initiates an editor-to-server request.
	pub async fn begin_c2s(
		&self,
		sid: SessionId,
		server_id: ServerId,
		req: AnyRequest,
		timeout: Duration,
	) -> Result<AnyResponse, ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(RoutingCmd::BeginC2s {
				sid,
				server_id,
				req,
				timeout,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Authoritatively cleans up a lost session.
	pub async fn session_lost(&self, sid: SessionId) {
		let _ = self.tx.send(RoutingCmd::SessionLost { sid }).await;
	}

	/// Delivers an editor notification.
	pub async fn lsp_send_notif(
		&self,
		sid: SessionId,
		server_id: ServerId,
		message: String,
	) -> Result<(), ErrorCode> {
		let (reply, rx) = oneshot::channel();
		self.tx
			.send(RoutingCmd::LspSendNotif {
				sid,
				server_id,
				message,
				reply,
			})
			.await
			.map_err(|_| ErrorCode::Internal)?;
		rx.await.map_err(|_| ErrorCode::Internal)?
	}

	/// Delivers a server notification.
	pub async fn server_notif(&self, server_id: ServerId, message: String) {
		let _ = self
			.tx
			.send(RoutingCmd::ServerNotif { server_id, message })
			.await;
	}

	/// Registers a document open from buffer sync.
	pub async fn lsp_doc_open(&self, uri: String, text: String) {
		let _ = self.tx.send(RoutingCmd::LspDocOpen { uri, text }).await;
	}

	/// Registers a document update from buffer sync.
	pub async fn lsp_doc_update(&self, uri: String, text: String) {
		let _ = self.tx.send(RoutingCmd::LspDocUpdate { uri, text }).await;
	}

	/// Registers a document close from buffer sync.
	pub async fn lsp_doc_close(&self, uri: String) {
		let _ = self.tx.send(RoutingCmd::LspDocClose { uri }).await;
	}

	/// Delivers a process exit signal.
	pub async fn server_exited(&self, server_id: ServerId, crashed: bool) {
		let _ = self
			.tx
			.send(RoutingCmd::ServerExited { server_id, crashed })
			.await;
	}

	/// Shutdown all managed servers.
	pub async fn terminate_all(&self) {
		let _ = self.tx.send(RoutingCmd::TerminateAll).await;
	}
}

/// Actor service for LSP routing and server lifecycle.
///
/// Manages a pool of LSP server processes, deduplicated by project configuration.
/// Ensures server-to-client requests are always routed to a deterministic leader session.
pub struct RoutingService {
	rx: mpsc::Receiver<RoutingCmd>,
	tx: mpsc::Sender<RoutingCmd>,
	servers: HashMap<ServerId, ServerEntry>,
	projects: HashMap<crate::core::ProjectKey, ServerId>,
	doc_servers: HashMap<String, ServerId>,
	pending_sync_docs: HashMap<String, String>,
	pending_lsp_closes: HashSet<String>,
	pending_s2c: HashMap<(ServerId, xeno_lsp::RequestId), PendingS2cReq>,
	pending_c2s: HashMap<(ServerId, xeno_lsp::RequestId), PendingC2sReq>,
	sessions: SessionHandle,
	knowledge: KnowledgeHandle,
	launcher: Arc<dyn LspLauncher>,
	next_server_id: u64,
	idle_lease: Duration,
}

impl RoutingService {
	/// Spawns the routing service actor.
	pub fn start(
		sessions: SessionHandle,
		knowledge: KnowledgeHandle,
		launcher: Arc<dyn LspLauncher>,
		idle_lease: Duration,
	) -> RoutingHandle {
		let (tx, rx) = mpsc::channel(256);
		let service = Self {
			rx,
			tx: tx.clone(),
			servers: HashMap::new(),
			projects: HashMap::new(),
			doc_servers: HashMap::new(),
			pending_sync_docs: HashMap::new(),
			pending_lsp_closes: HashSet::new(),
			pending_s2c: HashMap::new(),
			pending_c2s: HashMap::new(),
			sessions,
			knowledge,
			launcher,
			next_server_id: 0,
			idle_lease,
		};
		tokio::spawn(service.run());
		RoutingHandle::new(tx)
	}

	async fn run(mut self) {
		while let Some(cmd) = self.rx.recv().await {
			match cmd {
				RoutingCmd::LspStart { sid, config, reply } => {
					let res = self.handle_lsp_start(sid, config).await;
					let _ = reply.send(res);
				}
				RoutingCmd::BeginS2c {
					server_id,
					request_id,
					json,
					tx,
					reply,
				} => {
					let res = self.handle_begin_s2c(server_id, request_id, json, tx).await;
					let _ = reply.send(res);
				}
				RoutingCmd::CompleteS2c {
					sid,
					server_id,
					request_id,
					result,
					reply,
				} => {
					let _ =
						reply.send(self.handle_complete_s2c(sid, server_id, request_id, result));
				}
				RoutingCmd::CancelS2c {
					server_id,
					request_id,
				} => {
					self.handle_cancel_s2c(server_id, request_id);
				}
				RoutingCmd::BeginC2s {
					sid,
					server_id,
					req,
					timeout,
					reply,
				} => {
					self.handle_begin_c2s(sid, server_id, req, timeout, reply)
						.await;
				}
				RoutingCmd::C2sResp {
					server_id,
					resp,
					reply,
				} => {
					let result = self.handle_c2s_resp(server_id, resp);
					let _ = reply.send(result);
				}
				RoutingCmd::C2sTimeout {
					server_id,
					wire_id,
					reply,
				} => {
					let result = self.handle_c2s_timeout(server_id, wire_id);
					let _ = reply.send(result);
				}
				RoutingCmd::C2sSendFailed {
					server_id,
					wire_id,
					reply,
				} => {
					let result = self.handle_c2s_send_failed(server_id, wire_id);
					let _ = reply.send(result);
				}
				RoutingCmd::SessionLost { sid } => {
					self.handle_session_lost(sid).await;
				}
				RoutingCmd::ServerExited { server_id, crashed } => {
					self.handle_server_exit(server_id, crashed).await;
				}
				RoutingCmd::LeaseExpired {
					server_id,
					generation,
				} => {
					self.handle_lease_expiry(server_id, generation).await;
				}
				RoutingCmd::LspSendNotif {
					sid,
					server_id,
					message,
					reply,
				} => {
					let res = self.handle_lsp_send_notif(sid, server_id, message).await;
					let _ = reply.send(res);
				}
				RoutingCmd::ServerNotif { server_id, message } => {
					self.handle_server_notif(server_id, message).await;
				}
				RoutingCmd::LspDocOpen { uri, text } => {
					self.handle_lsp_doc_open(uri, text);
				}
				RoutingCmd::LspDocUpdate { uri, text } => {
					self.handle_lsp_doc_update(uri, text);
				}
				RoutingCmd::LspDocClose { uri } => {
					self.handle_lsp_doc_close(uri);
				}
				RoutingCmd::TerminateAll => {
					self.handle_terminate_all().await;
				}
			}
		}
	}

	async fn handle_lsp_start(
		&mut self,
		sid: SessionId,
		config: LspServerConfig,
	) -> Result<ServerId, ErrorCode> {
		if let Some(id) = self.find_server_for_project(&config)
			&& self.attach_session(id, sid)
		{
			return Ok(id);
		}

		let server_id = ServerId(self.next_server_id);
		self.next_server_id += 1;

		let instance = self
			.launcher
			.launch(RoutingHandle::new(self.tx.clone()), server_id, &config, sid)
			.await?;

		let project = crate::core::ProjectKey::from(&config);
		self.projects.insert(project.clone(), server_id);

		self.servers.insert(
			server_id,
			ServerEntry {
				instance,
				project,
				attached: [sid].into(),
				leader: sid,
				docs: crate::core::text_sync::DocRegistry::default(),
				lsp_docs: HashMap::new(),
				lease_gen: 0,
				doc_owners: crate::core::text_sync::DocOwnerRegistry::default(),
				next_wire_req_id: 1,
			},
		);

		if let Some(cwd) = config.cwd.as_ref() {
			self.knowledge
				.spawn_project_crawl(std::path::PathBuf::from(cwd));
		}

		Ok(server_id)
	}

	fn find_server_for_project(&self, config: &LspServerConfig) -> Option<ServerId> {
		let key = crate::core::ProjectKey::from(config);
		self.projects.get(&key).cloned()
	}

	fn attach_session(&mut self, server_id: ServerId, session_id: SessionId) -> bool {
		let Some(server) = self.servers.get_mut(&server_id) else {
			return false;
		};
		server.attached.insert(session_id);
		if let Some(&min_id) = server.attached.iter().min() {
			server.leader = min_id;
		}
		server.lease_gen += 1;
		let cached = server
			.docs
			.diagnostics_by_uri
			.iter()
			.map(|(uri, diag)| Event::LspDiagnostics {
				server_id,
				doc_id: server.docs.by_uri.get(uri).map(|(id, _)| *id),
				uri: uri.clone(),
				version: diag.version,
				diagnostics: diag.diagnostics.clone(),
			})
			.collect::<Vec<_>>();
		if !cached.is_empty() {
			let sessions = self.sessions.clone();
			tokio::spawn(async move {
				for event in cached {
					sessions
						.send(session_id, xeno_broker_proto::types::IpcFrame::Event(event))
						.await;
				}
			});
		}
		Self::refresh_lsp_docs(server);
		true
	}

	async fn handle_begin_s2c(
		&mut self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		json: String,
		tx: oneshot::Sender<crate::core::LspReplyResult>,
	) -> Result<(), xeno_lsp::ResponseError> {
		let leader = {
			let server = self.servers.get(&server_id).ok_or_else(|| {
				xeno_lsp::ResponseError::new(
					xeno_lsp::ErrorCode::INTERNAL_ERROR,
					"Server not found",
				)
			})?;
			if server.attached.is_empty() {
				return Err(xeno_lsp::ResponseError::new(
					xeno_lsp::ErrorCode::METHOD_NOT_FOUND,
					"No sessions attached",
				));
			}
			server.leader
		};

		self.pending_s2c.insert(
			(server_id, request_id.clone()),
			PendingS2cReq {
				responder: leader,
				tx,
			},
		);

		let event = Event::LspRequest {
			server_id,
			message: json,
		};
		if !self
			.sessions
			.send_checked(leader, xeno_broker_proto::types::IpcFrame::Event(event))
			.await
		{
			self.pending_s2c.remove(&(server_id, request_id));
			return Err(xeno_lsp::ResponseError::new(
				xeno_lsp::ErrorCode::INTERNAL_ERROR,
				"Leader session lost",
			));
		}

		Ok(())
	}

	fn handle_complete_s2c(
		&mut self,
		sid: SessionId,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		result: crate::core::LspReplyResult,
	) -> bool {
		if let Some(req) = self.pending_s2c.get(&(server_id, request_id.clone()))
			&& req.responder == sid
			&& let Some(req) = self.pending_s2c.remove(&(server_id, request_id))
		{
			let _ = req.tx.send(result);
			return true;
		}
		false
	}

	fn handle_cancel_s2c(&mut self, server_id: ServerId, request_id: xeno_lsp::RequestId) {
		if let Some(req) = self.pending_s2c.remove(&(server_id, request_id)) {
			let _ = req.tx.send(Err(xeno_lsp::ResponseError::new(
				xeno_lsp::ErrorCode::REQUEST_CANCELLED,
				"cancelled",
			)));
		}
	}

	async fn handle_session_lost(&mut self, sid: SessionId) {
		let affected: Vec<ServerId> = self
			.servers
			.iter()
			.filter(|(_, s)| s.attached.contains(&sid))
			.map(|(id, _)| *id)
			.collect();

		for server_id in affected {
			let mut schedule_lease = false;
			let mut current_gen = 0;
			let mut removed_uris = Vec::new();

			if let Some(server) = self.servers.get_mut(&server_id) {
				server.attached.remove(&sid);
				if server.leader == sid
					&& let Some(&new_leader) = server.attached.iter().min()
				{
					server.leader = new_leader;
				}
				let mut to_remove = Vec::new();
				for (uri, state) in &mut server.doc_owners.by_uri {
					state.open_refcounts.remove(&sid);
					if state.open_refcounts.is_empty() {
						to_remove.push(uri.clone());
						continue;
					}
					if (state.owner == sid || !server.attached.contains(&state.owner))
						&& let Some(&next) = state.open_refcounts.keys().min()
					{
						state.owner = next;
					}
				}
				for uri in to_remove {
					server.doc_owners.by_uri.remove(&uri);
					server.docs.remove(&uri);
					removed_uris.push(uri);
				}
				if server.attached.is_empty() {
					server.lease_gen += 1;
					schedule_lease = true;
					current_gen = server.lease_gen;
				}
				#[cfg(debug_assertions)]
				debug_assert!(
					server.attached.is_empty() || server.attached.contains(&server.leader)
				);
			}

			for uri in removed_uris {
				self.handle_lsp_doc_close(uri);
			}

			// Cancel responder requests
			let to_cancel: Vec<_> = self
				.pending_s2c
				.iter()
				.filter(|((s_id, _), req)| *s_id == server_id && req.responder == sid)
				.map(|(k, _)| k.clone())
				.collect();
			for (s_id, rid) in to_cancel {
				self.handle_cancel_s2c(s_id, rid);
			}

			if schedule_lease {
				// Cancel ALL remaining for empty server
				let to_cancel_all: Vec<_> = self
					.pending_s2c
					.keys()
					.filter(|(s_id, _)| *s_id == server_id)
					.cloned()
					.collect();
				for (s_id, rid) in to_cancel_all {
					self.handle_cancel_s2c(s_id, rid);
				}

				let tx = self.tx.clone();
				let duration = self.idle_lease;
				tokio::spawn(async move {
					tokio::time::sleep(duration).await;
					let _ = tx
						.send(RoutingCmd::LeaseExpired {
							server_id,
							generation: current_gen,
						})
						.await;
				});
			}
		}
		self.pending_c2s.retain(|_, req| req.origin_session != sid);
	}

	async fn handle_lsp_send_notif(
		&mut self,
		sid: SessionId,
		server_id: ServerId,
		message: String,
	) -> Result<(), ErrorCode> {
		let notif: xeno_lsp::AnyNotification =
			serde_json::from_str(&message).map_err(|_| ErrorCode::InvalidArgs)?;

		let is_doc_sync = matches!(
			notif.method.as_str(),
			"textDocument/didOpen" | "textDocument/didChange" | "textDocument/didClose"
		);

		if is_doc_sync {
			let decision = {
				let server = self
					.servers
					.get_mut(&server_id)
					.ok_or(ErrorCode::ServerNotFound)?;
				Self::gate_text_sync(sid, server, &notif)
			};

			match decision {
				DocGateDecision::RejectNotOwner => return Err(ErrorCode::NotPreferredOwner),
				DocGateDecision::DropSilently => return Ok(()),
				DocGateDecision::Forward => {
					self.handle_session_text_sync(server_id, &notif);
					return Ok(());
				}
			}
		}

		let server = self
			.servers
			.get_mut(&server_id)
			.ok_or(ErrorCode::ServerNotFound)?;
		let _ = server
			.instance
			.lsp_tx
			.send(xeno_rpc::MainLoopEvent::Outgoing(
				xeno_lsp::Message::Notification(notif),
			));
		Ok(())
	}

	async fn handle_begin_c2s(
		&mut self,
		sid: SessionId,
		server_id: ServerId,
		mut req: AnyRequest,
		timeout: Duration,
		reply: oneshot::Sender<Result<AnyResponse, ErrorCode>>,
	) {
		let Some(server) = self.servers.get_mut(&server_id) else {
			let _ = reply.send(Err(ErrorCode::ServerNotFound));
			return;
		};
		if !server.attached.contains(&sid) {
			let _ = reply.send(Err(ErrorCode::ServerNotFound));
			return;
		};

		let origin_id = req.id.clone();
		let wire_id = RequestId::String(format!("b:{}:{}", server_id.0, server.next_wire_req_id));
		server.next_wire_req_id += 1;
		req.id = wire_id.clone();

		let (tx, rx) = oneshot::channel();
		if server
			.instance
			.lsp_tx
			.send(MainLoopEvent::OutgoingRequest(req, tx))
			.is_err()
		{
			let _ = reply.send(Err(ErrorCode::Internal));
			return;
		}

		self.pending_c2s.insert(
			(server_id, wire_id.clone()),
			PendingC2sReq {
				origin_session: sid,
				origin_id,
			},
		);

		let routing_tx = self.tx.clone();
		tokio::spawn(async move {
			match tokio::time::timeout(timeout, rx).await {
				Ok(Ok(resp)) => {
					let _ = routing_tx
						.send(RoutingCmd::C2sResp {
							server_id,
							resp,
							reply,
						})
						.await;
				}
				Ok(Err(_)) => {
					let _ = routing_tx
						.send(RoutingCmd::C2sSendFailed {
							server_id,
							wire_id,
							reply,
						})
						.await;
				}
				Err(_) => {
					let _ = routing_tx
						.send(RoutingCmd::C2sTimeout {
							server_id,
							wire_id,
							reply,
						})
						.await;
				}
			}
		});
	}

	fn handle_c2s_resp(
		&mut self,
		server_id: ServerId,
		mut resp: AnyResponse,
	) -> Result<AnyResponse, ErrorCode> {
		let Some(pending) = self.pending_c2s.remove(&(server_id, resp.id.clone())) else {
			return Err(ErrorCode::RequestNotFound);
		};
		resp.id = pending.origin_id;
		Ok(resp)
	}

	fn handle_c2s_timeout(
		&mut self,
		server_id: ServerId,
		wire_id: RequestId,
	) -> Result<AnyResponse, ErrorCode> {
		if self.pending_c2s.remove(&(server_id, wire_id)).is_none() {
			return Err(ErrorCode::RequestNotFound);
		}
		Err(ErrorCode::Timeout)
	}

	fn handle_c2s_send_failed(
		&mut self,
		server_id: ServerId,
		wire_id: RequestId,
	) -> Result<AnyResponse, ErrorCode> {
		if self.pending_c2s.remove(&(server_id, wire_id)).is_none() {
			return Err(ErrorCode::RequestNotFound);
		}
		Err(ErrorCode::Internal)
	}

	fn gate_text_sync(
		session_id: SessionId,
		server: &mut ServerEntry,
		notif: &xeno_lsp::AnyNotification,
	) -> DocGateDecision {
		let method = notif.method.as_str();
		if !matches!(
			method,
			"textDocument/didOpen" | "textDocument/didChange" | "textDocument/didClose"
		) {
			return DocGateDecision::Forward;
		}

		let doc = notif.params.get("textDocument").and_then(|d| d.as_object());
		let uri = doc.and_then(|d| d.get("uri")).and_then(|u| u.as_str());
		let version = doc
			.and_then(|d| d.get("version"))
			.and_then(|v| v.as_u64())
			.map(|v| v as u32)
			.unwrap_or(0);

		let Some(uri) = uri else {
			return DocGateDecision::RejectNotOwner;
		};

		match method {
			"textDocument/didOpen" => match server.doc_owners.by_uri.get_mut(uri) {
				None => {
					server.doc_owners.by_uri.insert(
						uri.to_string(),
						DocOwnerState {
							owner: session_id,
							open_refcounts: [(session_id, 1)].into(),
							last_version: version,
						},
					);
					DocGateDecision::Forward
				}
				Some(os) => {
					*os.open_refcounts.entry(session_id).or_insert(0) += 1;
					if !server.attached.contains(&os.owner)
						|| !os.open_refcounts.contains_key(&os.owner)
					{
						os.owner = session_id;
					}
					DocGateDecision::DropSilently
				}
			},
			"textDocument/didChange" => match server.doc_owners.by_uri.get_mut(uri) {
				None => DocGateDecision::RejectNotOwner,
				Some(os) => {
					if session_id == os.owner
						|| !server.attached.contains(&os.owner)
						|| !os.open_refcounts.contains_key(&os.owner)
						|| os.open_refcounts.contains_key(&session_id)
					{
						os.owner = session_id;
						os.last_version = version;
						DocGateDecision::Forward
					} else {
						DocGateDecision::RejectNotOwner
					}
				}
			},
			"textDocument/didClose" => match server.doc_owners.by_uri.get_mut(uri) {
				None => DocGateDecision::RejectNotOwner,
				Some(os) => {
					if let Some(c) = os.open_refcounts.get_mut(&session_id) {
						if *c > 0 {
							*c -= 1;
						}
						if *c == 0 {
							os.open_refcounts.remove(&session_id);
						}
					}
					if session_id == os.owner
						&& !os.open_refcounts.is_empty()
						&& let Some(&next) = os.open_refcounts.keys().min()
					{
						os.owner = next;
					}
					if os.open_refcounts.values().sum::<u32>() == 0 {
						server.doc_owners.by_uri.remove(uri);
						server.docs.remove(uri);
						DocGateDecision::Forward
					} else {
						DocGateDecision::DropSilently
					}
				}
			},
			_ => unreachable!(),
		}
	}

	fn handle_session_text_sync(&mut self, server_id: ServerId, notif: &AnyNotification) {
		match notif.method.as_str() {
			"textDocument/didOpen" => {
				let Some(doc) = notif.params.get("textDocument").and_then(|d| d.as_object()) else {
					return;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return;
				};
				let language_id = doc
					.get("languageId")
					.and_then(|v| v.as_str())
					.map(|v| v.to_string());
				let text = doc
					.get("text")
					.and_then(|v| v.as_str())
					.unwrap_or("")
					.to_string();
				let version = doc
					.get("version")
					.and_then(|v| v.as_u64())
					.map(|v| v as u32)
					.unwrap_or(1);

				self.pending_lsp_closes.remove(uri);
				let pending = self.pending_sync_docs.remove(uri);
				self.doc_servers.insert(uri.to_string(), server_id);
				let Some(server) = self.servers.get_mut(&server_id) else {
					self.doc_servers.remove(uri);
					if let Some(text) = pending {
						self.pending_sync_docs.insert(uri.to_string(), text);
					}
					return;
				};

				let mut action = None;
				let mut pending_insert = None;
				{
					let entry = server
						.lsp_docs
						.entry(uri.to_string())
						.or_insert(LspDocState {
							language_id: None,
							text: String::new(),
							version: 1,
							open: false,
						});
					if let Some(lang) = language_id {
						entry.language_id = Some(lang);
					}
					if !text.is_empty() || entry.text.is_empty() {
						entry.text = text;
					}
					if version > 0 {
						entry.version = version;
					}

					if let Some(text) = pending {
						let text_changed = text != entry.text;
						if text_changed {
							entry.text = text;
						}
						if entry.open {
							if text_changed {
								entry.version = entry.version.saturating_add(1);
								action = Some(LspDocAction::Change {
									uri: uri.to_string(),
									version: entry.version,
									text: entry.text.clone(),
								});
							}
						} else if let Some(language_id) = entry.language_id.clone() {
							let version = entry.version.max(1);
							entry.version = version;
							entry.open = true;
							action = Some(LspDocAction::Open {
								uri: uri.to_string(),
								language_id,
								version,
								text: entry.text.clone(),
							});
						} else {
							pending_insert = Some(entry.text.clone());
						}
					} else if !entry.open
						&& let Some(language_id) = entry.language_id.clone()
					{
						let version = entry.version.max(1);
						entry.version = version;
						entry.open = true;
						action = Some(LspDocAction::Open {
							uri: uri.to_string(),
							language_id,
							version,
							text: entry.text.clone(),
						});
					}
				}
				if let Some(action) = action {
					Self::apply_lsp_doc_action(server, action);
				}
				if let Some(text) = pending_insert {
					self.pending_sync_docs.insert(uri.to_string(), text);
				}
			}
			"textDocument/didChange" => {
				let Some(doc) = notif.params.get("textDocument").and_then(|d| d.as_object()) else {
					return;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return;
				};
				let version = doc
					.get("version")
					.and_then(|v| v.as_u64())
					.map(|v| v as u32);
				let Some(changes_val) = notif
					.params
					.get("contentChanges")
					.and_then(|v| v.as_array())
				else {
					return;
				};
				let changes: Vec<LspContentChange> = changes_val
					.iter()
					.filter_map(|value| {
						let obj = value.as_object()?;
						let text = obj.get("text")?.as_str()?.to_string();
						let range = obj.get("range").and_then(|range_val| {
							let range_obj = range_val.as_object()?;
							let start_obj = range_obj.get("start")?.as_object()?;
							let end_obj = range_obj.get("end")?.as_object()?;
							let start = LspPosition {
								line: start_obj.get("line")?.as_u64()? as u32,
								character: start_obj.get("character")?.as_u64()? as u32,
							};
							let end = LspPosition {
								line: end_obj.get("line")?.as_u64()? as u32,
								character: end_obj.get("character")?.as_u64()? as u32,
							};
							Some(LspRange { start, end })
						});
						Some(LspContentChange { range, text })
					})
					.collect();
				if changes.is_empty() {
					return;
				}

				self.pending_lsp_closes.remove(uri);
				self.doc_servers.insert(uri.to_string(), server_id);
				let Some(server) = self.servers.get_mut(&server_id) else {
					return;
				};
				let mut action = None;
				let mut pending_insert = None;
				{
					let entry = server
						.lsp_docs
						.entry(uri.to_string())
						.or_insert(LspDocState {
							language_id: None,
							text: String::new(),
							version: 1,
							open: false,
						});

					let Some(new_text) = apply_content_changes(&entry.text, &changes) else {
						return;
					};
					let text_changed = new_text != entry.text;

					if text_changed {
						entry.text = new_text;
					}

					if entry.open {
						if text_changed {
							let next_version =
								version.unwrap_or_else(|| entry.version.saturating_add(1));
							entry.version = next_version.max(1);
							action = Some(LspDocAction::Change {
								uri: uri.to_string(),
								version: entry.version,
								text: entry.text.clone(),
							});
						}
					} else if let Some(language_id) = entry.language_id.clone() {
						let next_version = version.unwrap_or(entry.version.max(1));
						entry.version = next_version.max(1);
						entry.open = true;
						action = Some(LspDocAction::Open {
							uri: uri.to_string(),
							language_id,
							version: entry.version,
							text: entry.text.clone(),
						});
					} else if text_changed {
						pending_insert = Some(entry.text.clone());
					}
				}
				if let Some(action) = action {
					Self::apply_lsp_doc_action(server, action);
				}
				if let Some(text) = pending_insert {
					self.pending_sync_docs.insert(uri.to_string(), text);
				}
			}
			"textDocument/didClose" => {
				let Some(doc) = notif.params.get("textDocument").and_then(|d| d.as_object()) else {
					return;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return;
				};
				self.pending_sync_docs.remove(uri);
				self.pending_lsp_closes.remove(uri);
				self.doc_servers.remove(uri);

				let Some(server) = self.servers.get_mut(&server_id) else {
					return;
				};
				let mut send_close = false;
				{
					if let Some(state) = server.lsp_docs.get_mut(uri)
						&& state.open
					{
						state.open = false;
						send_close = true;
					}
				}
				if send_close {
					Self::send_did_close(server, uri);
				}
			}
			_ => {}
		}
	}

	fn handle_lsp_doc_open(&mut self, uri: String, text: String) {
		self.pending_lsp_closes.remove(&uri);
		let Some(server_id) = self.doc_servers.get(&uri).copied() else {
			self.pending_sync_docs.insert(uri, text);
			return;
		};
		let Some(server) = self.servers.get_mut(&server_id) else {
			self.pending_sync_docs.insert(uri, text);
			return;
		};
		let mut action = None;
		let mut pending_insert = None;
		{
			let entry = server.lsp_docs.entry(uri.clone()).or_insert(LspDocState {
				language_id: None,
				text: text.clone(),
				version: 1,
				open: false,
			});
			let text_changed = entry.text != text;
			if text_changed {
				entry.text = text;
			}
			if entry.open {
				if text_changed {
					entry.version = entry.version.saturating_add(1);
					action = Some(LspDocAction::Change {
						uri: uri.clone(),
						version: entry.version,
						text: entry.text.clone(),
					});
				}
			} else if let Some(language_id) = entry.language_id.clone() {
				let version = entry.version.max(1);
				entry.version = version;
				entry.open = true;
				action = Some(LspDocAction::Open {
					uri: uri.clone(),
					language_id,
					version,
					text: entry.text.clone(),
				});
			} else {
				pending_insert = Some(entry.text.clone());
			}
		}
		if let Some(action) = action {
			Self::apply_lsp_doc_action(server, action);
		}
		if let Some(text) = pending_insert {
			self.pending_sync_docs.insert(uri, text);
		}
	}

	fn handle_lsp_doc_update(&mut self, uri: String, text: String) {
		self.pending_lsp_closes.remove(&uri);
		let Some(server_id) = self.doc_servers.get(&uri).copied() else {
			self.pending_sync_docs.insert(uri, text);
			return;
		};
		let Some(server) = self.servers.get_mut(&server_id) else {
			self.pending_sync_docs.insert(uri, text);
			return;
		};
		let mut action = None;
		let mut pending_insert = None;
		{
			let entry = server.lsp_docs.entry(uri.clone()).or_insert(LspDocState {
				language_id: None,
				text: text.clone(),
				version: 1,
				open: false,
			});
			let text_changed = entry.text != text;
			if text_changed {
				entry.text = text;
			}
			if entry.open {
				if text_changed {
					entry.version = entry.version.saturating_add(1);
					action = Some(LspDocAction::Change {
						uri: uri.clone(),
						version: entry.version,
						text: entry.text.clone(),
					});
				}
			} else if let Some(language_id) = entry.language_id.clone() {
				let version = entry.version.max(1);
				entry.version = version;
				entry.open = true;
				action = Some(LspDocAction::Open {
					uri: uri.clone(),
					language_id,
					version,
					text: entry.text.clone(),
				});
			} else {
				pending_insert = Some(entry.text.clone());
			}
		}
		if let Some(action) = action {
			Self::apply_lsp_doc_action(server, action);
		}
		if let Some(text) = pending_insert {
			self.pending_sync_docs.insert(uri, text);
		}
	}

	fn handle_lsp_doc_close(&mut self, uri: String) {
		self.pending_sync_docs.remove(&uri);
		let Some(server_id) = self.doc_servers.get(&uri).copied() else {
			return;
		};
		let Some(server) = self.servers.get_mut(&server_id) else {
			return;
		};
		if server.doc_owners.by_uri.contains_key(&uri) {
			self.pending_lsp_closes.insert(uri);
			return;
		}
		self.pending_lsp_closes.remove(&uri);
		self.doc_servers.remove(&uri);
		if let Some(state) = server.lsp_docs.remove(&uri)
			&& state.open
		{
			Self::send_did_close(server, &uri);
		}
		server.docs.remove(&uri);
	}

	fn refresh_lsp_docs(server: &mut ServerEntry) {
		let uris: Vec<String> = server.lsp_docs.keys().cloned().collect();
		for uri in uris {
			let mut action = None;
			if let Some(state) = server.lsp_docs.get_mut(&uri)
				&& state.open
			{
				state.version = state.version.saturating_add(1);
				action = Some(LspDocAction::Change {
					uri: uri.clone(),
					version: state.version,
					text: state.text.clone(),
				});
			}
			if let Some(action) = action {
				Self::apply_lsp_doc_action(server, action);
			}
		}
	}

	fn apply_lsp_doc_action(server: &mut ServerEntry, action: LspDocAction) {
		match action {
			LspDocAction::Open {
				uri,
				language_id,
				version,
				text,
			} => {
				Self::send_did_open(server, &uri, &language_id, version, &text);
				server.docs.update(uri, version);
			}
			LspDocAction::Change { uri, version, text } => {
				Self::send_did_change(server, &uri, version, &text);
				server.docs.update(uri, version);
			}
		}
	}

	fn send_did_open(server: &ServerEntry, uri: &str, language_id: &str, version: u32, text: &str) {
		let notif = AnyNotification::new(
			"textDocument/didOpen",
			serde_json::json!({
				"textDocument": {
					"uri": uri,
					"languageId": language_id,
					"version": version,
					"text": text
				}
			}),
		);
		let _ = server
			.instance
			.lsp_tx
			.send(MainLoopEvent::Outgoing(Message::Notification(notif)));
	}

	fn send_did_change(server: &ServerEntry, uri: &str, version: u32, text: &str) {
		let notif = AnyNotification::new(
			"textDocument/didChange",
			serde_json::json!({
				"textDocument": {
					"uri": uri,
					"version": version
				},
				"contentChanges": [{
					"text": text
				}]
			}),
		);
		let _ = server
			.instance
			.lsp_tx
			.send(MainLoopEvent::Outgoing(Message::Notification(notif)));
	}

	fn send_did_close(server: &ServerEntry, uri: &str) {
		let notif = AnyNotification::new(
			"textDocument/didClose",
			serde_json::json!({
				"textDocument": {
					"uri": uri
				}
			}),
		);
		let _ = server
			.instance
			.lsp_tx
			.send(MainLoopEvent::Outgoing(Message::Notification(notif)));
	}

	async fn handle_server_notif(&mut self, server_id: ServerId, message: String) {
		let (attached, event) = {
			let Some(server) = self.servers.get_mut(&server_id) else {
				return;
			};

			let attached: Vec<_> = server.attached.iter().cloned().collect();
			let mut diagnostics_event = None;

			if let Ok(msg) = serde_json::from_str::<xeno_lsp::Message>(&message)
				&& let xeno_lsp::Message::Notification(notif) = msg
				&& notif.method == "textDocument/publishDiagnostics"
				&& let Some(uri) = notif.params.get("uri").and_then(|u| u.as_str())
				&& let Some(diagnostics) = notif.params.get("diagnostics")
				&& let Ok(diagnostics) = serde_json::to_string(diagnostics)
			{
				let version = notif
					.params
					.get("version")
					.and_then(|v| v.as_u64())
					.map(|v| v as u32);
				server
					.docs
					.update_diagnostics(uri.to_string(), version, diagnostics.clone());
				let doc_id = server.docs.by_uri.get(uri).map(|(id, _)| *id);
				diagnostics_event = Some(Event::LspDiagnostics {
					server_id,
					doc_id,
					uri: uri.to_string(),
					version,
					diagnostics,
				});
			}

			let event = diagnostics_event.unwrap_or(Event::LspMessage { server_id, message });
			(attached, event)
		};
		if attached.is_empty() {
			return;
		}
		self.sessions
			.broadcast(
				attached,
				xeno_broker_proto::types::IpcFrame::Event(event),
				None,
			)
			.await;
	}

	async fn handle_terminate_all(&mut self) {
		let ids: Vec<_> = self.servers.keys().cloned().collect();
		for id in ids {
			self.handle_server_exit(id, false).await;
		}
		for (_, req) in std::mem::take(&mut self.pending_s2c) {
			let _ = req.tx.send(Err(xeno_lsp::ResponseError::new(
				xeno_lsp::ErrorCode::REQUEST_CANCELLED,
				"shutting down",
			)));
		}
		self.pending_c2s.clear();
	}

	async fn handle_lease_expiry(&mut self, server_id: ServerId, generation: u64) {
		let s = self.servers.get(&server_id);
		let should = s
			.map(|s| s.lease_gen == generation && s.attached.is_empty())
			.unwrap_or(false);
		if should
			&& !self.pending_s2c.keys().any(|(sid, _)| *sid == server_id)
			&& !self.pending_c2s.keys().any(|(sid, _)| *sid == server_id)
		{
			self.handle_server_exit(server_id, false).await;
		}
	}

	async fn handle_server_exit(&mut self, server_id: ServerId, crashed: bool) {
		let keys: Vec<_> = self
			.pending_s2c
			.keys()
			.filter(|(sid, _)| *sid == server_id)
			.cloned()
			.collect();
		for k in keys {
			if let Some(r) = self.pending_s2c.remove(&k) {
				let _ = r.tx.send(Err(xeno_lsp::ResponseError::new(
					xeno_lsp::ErrorCode::REQUEST_CANCELLED,
					"exited",
				)));
			}
		}
		self.pending_c2s.retain(|(sid, _), _| *sid != server_id);

		if let Some(server) = self.servers.remove(&server_id) {
			for uri in server.lsp_docs.keys() {
				self.doc_servers.remove(uri);
				self.pending_sync_docs.remove(uri);
			}
			self.projects.remove(&server.project);
			let attached = server.attached.into_iter().collect();
			let status = if crashed {
				LspServerStatus::Crashed
			} else {
				LspServerStatus::Stopped
			};
			self.sessions
				.broadcast(
					attached,
					xeno_broker_proto::types::IpcFrame::Event(Event::LspStatus {
						server_id,
						status,
					}),
					None,
				)
				.await;
			tokio::spawn(async move {
				server.instance.terminate().await;
			});
		}
	}
}
