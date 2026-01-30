//! Shared broker core state and session management.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;
use xeno_broker_proto::types::{
	DocId, Event, IpcFrame, LspServerConfig, Request, Response, ServerId, SessionId,
};
use xeno_rpc::{MainLoopEvent, PeerSocket};

/// Sink for sending events to a connected session.
pub type SessionSink = PeerSocket<IpcFrame, Request, Response>;

/// Sink for sending messages to an LSP server.
pub type LspTx = PeerSocket<xeno_lsp::Message, xeno_lsp::AnyRequest, xeno_lsp::AnyResponse>;

/// Result of a server-to-editor request.
pub type LspReplyResult = Result<serde_json::Value, xeno_lsp::ResponseError>;

/// Configuration for the broker core.
#[derive(Debug, Clone)]
pub struct BrokerConfig {
	/// Duration to keep an idle server alive after all sessions detach.
	pub idle_lease: Duration,
}

impl Default for BrokerConfig {
	fn default() -> Self {
		Self {
			idle_lease: Duration::from_secs(300), // 5 minutes default
		}
	}
}

/// Shared state for the broker.
///
/// Tracks active editor sessions, running LSP server instances, and
/// server->client request routing state.
#[derive(Debug)]
pub struct BrokerCore {
	state: Mutex<BrokerState>,
	next_server_id: AtomicU64,
	config: BrokerConfig,
}

impl Default for BrokerCore {
	fn default() -> Self {
		Self {
			state: Mutex::new(BrokerState::default()),
			next_server_id: AtomicU64::new(0),
			config: BrokerConfig::default(),
		}
	}
}

#[derive(Debug, Default)]
struct BrokerState {
	sessions: HashMap<SessionId, SessionEntry>,
	servers: HashMap<ServerId, ServerEntry>,
	projects: HashMap<ProjectKey, ServerId>,
	pending_client_reqs: HashMap<(ServerId, xeno_lsp::RequestId), PendingClientReq>,
}

/// Unique key for deduplicating LSP servers by project.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ProjectKey {
	/// The command used to start the server.
	pub command: String,
	/// Arguments passed to the command.
	pub args: Vec<String>,
	/// The project root directory.
	pub cwd: String,
}

impl From<&LspServerConfig> for ProjectKey {
	fn from(cfg: &LspServerConfig) -> Self {
		Self {
			command: cfg.command.clone(),
			args: cfg.args.clone(),
			cwd: cfg.cwd.clone().unwrap_or_default(),
		}
	}
}

#[derive(Debug)]
struct SessionEntry {
	sink: SessionSink,
	attached: HashSet<ServerId>,
}

#[derive(Debug)]
struct ServerEntry {
	instance: LspInstance,
	project: ProjectKey,
	attached: HashSet<SessionId>,
	leader: SessionId,
	docs: DocRegistry,
	lease_gen: u64,
}

#[derive(Debug)]
struct PendingClientReq {
	responder: SessionId,
	tx: oneshot::Sender<LspReplyResult>,
}

impl BrokerCore {
	/// Create a new broker core instance with default configuration.
	#[must_use]
	pub fn new() -> Arc<Self> {
		Self::new_with_config(BrokerConfig::default())
	}

	/// Create a new broker core instance with custom configuration.
	#[must_use]
	pub fn new_with_config(config: BrokerConfig) -> Arc<Self> {
		Arc::new(Self {
			state: Mutex::new(BrokerState::default()),
			next_server_id: AtomicU64::new(0),
			config,
		})
	}

	/// Register an editor session with its outbound event sink.
	pub fn register_session(&self, session_id: SessionId, sink: SessionSink) {
		let mut state = self.state.lock().unwrap();
		state.sessions.insert(
			session_id,
			SessionEntry {
				sink,
				attached: HashSet::new(),
			},
		);
	}

	/// Unregister an editor session and detach from all servers.
	pub fn unregister_session(self: &Arc<Self>, session_id: SessionId) {
		let mut servers_to_detach = Vec::new();
		{
			let mut state = self.state.lock().unwrap();
			if let Some(session) = state.sessions.remove(&session_id) {
				servers_to_detach.extend(session.attached);
			}
		}

		for server_id in servers_to_detach {
			self.detach_session(server_id, session_id);
		}

		// Drop pending requests where this session was the responder.
		let mut state = self.state.lock().unwrap();
		state
			.pending_client_reqs
			.retain(|_, req| req.responder != session_id);
	}

	/// Look up an existing server for a project or return None.
	pub fn find_server_for_project(&self, config: &LspServerConfig) -> Option<ServerId> {
		let key = ProjectKey::from(config);
		let state = self.state.lock().unwrap();
		state.projects.get(&key).cloned()
	}

	/// Attach a session to an existing server.
	pub fn attach_session(&self, server_id: ServerId, session_id: SessionId) -> bool {
		let mut state = self.state.lock().unwrap();
		let Some(server) = state.servers.get_mut(&server_id) else {
			return false;
		};

		server.attached.insert(session_id);
		if server.attached.len() == 1 {
			server.leader = session_id;
		}

		// Invalidate any pending lease tasks.
		server.lease_gen += 1;

		if let Some(session) = state.sessions.get_mut(&session_id) {
			session.attached.insert(server_id);
		}

		true
	}

	/// Detach a session from a server.
	///
	/// If this was the last session, an idle lease is scheduled to eventually
	/// terminate the server.
	pub fn detach_session(self: &Arc<Self>, server_id: ServerId, session_id: SessionId) {
		let mut maybe_schedule: Option<(u64, tokio::time::Instant)> = None;

		{
			let mut state = self.state.lock().unwrap();

			// Update server side.
			if let Some(server) = state.servers.get_mut(&server_id) {
				server.attached.remove(&session_id);

				if server.leader == session_id
					&& let Some(&new_leader) = server.attached.iter().next()
				{
					server.leader = new_leader;
				}

				if server.attached.is_empty() {
					server.lease_gen += 1;
					let generation = server.lease_gen;
					let deadline = tokio::time::Instant::now() + self.config.idle_lease;
					maybe_schedule = Some((generation, deadline));
				}
			}

			// Update session side.
			if let Some(session) = state.sessions.get_mut(&session_id) {
				session.attached.remove(&server_id);
			}
		}

		if let Some((generation, deadline)) = maybe_schedule {
			let core = self.clone();
			tokio::spawn(async move {
				tokio::time::sleep_until(deadline).await;
				core.check_lease_expiry(server_id, generation);
			});
		}
	}

	/// Check if a lease has expired and terminate the server if so.
	fn check_lease_expiry(self: &Arc<Self>, server_id: ServerId, generation: u64) {
		let maybe_instance = {
			let mut state = self.state.lock().unwrap();

			let Some(server) = state.servers.get(&server_id) else {
				return;
			};

			if server.lease_gen != generation || !server.attached.is_empty() {
				return;
			}

			let server = state.servers.remove(&server_id).unwrap();
			state.projects.remove(&server.project);

			// Drop any pending server->client requests for this server.
			state
				.pending_client_reqs
				.retain(|(sid, _), _| *sid != server_id);

			Some(server.instance)
		};

		if let Some(instance) = maybe_instance {
			{
				let mut current = instance.status.lock().unwrap();
				*current = xeno_broker_proto::types::LspServerStatus::Stopped;
			}
			tokio::spawn(async move {
				instance.terminate().await;
			});
		}
	}

	/// Register a new running LSP instance.
	pub fn register_server(
		&self,
		server_id: ServerId,
		instance: LspInstance,
		config: &LspServerConfig,
		owner: SessionId,
	) {
		let project = ProjectKey::from(config);
		let mut state = self.state.lock().unwrap();

		state.projects.insert(project.clone(), server_id);

		let mut attached = HashSet::new();
		attached.insert(owner);

		state.servers.insert(
			server_id,
			ServerEntry {
				instance,
				project,
				attached,
				leader: owner,
				docs: DocRegistry::default(),
				lease_gen: 0,
			},
		);

		if let Some(session) = state.sessions.get_mut(&owner) {
			session.attached.insert(server_id);
		}
	}

	/// Unregister an LSP instance and release its resources.
	pub fn unregister_server(&self, server_id: ServerId) {
		let maybe_instance = {
			let mut state = self.state.lock().unwrap();

			let Some(server) = state.servers.remove(&server_id) else {
				return;
			};

			state.projects.remove(&server.project);

			for session_id in &server.attached {
				if let Some(session) = state.sessions.get_mut(session_id) {
					session.attached.remove(&server_id);
				}
			}

			state
				.pending_client_reqs
				.retain(|(sid, _), _| *sid != server_id);

			Some(server.instance)
		};

		if let Some(instance) = maybe_instance {
			{
				let mut current = instance.status.lock().unwrap();
				*current = xeno_broker_proto::types::LspServerStatus::Stopped;
			}
			tokio::spawn(async move {
				instance.terminate().await;
			});
		}
	}

	/// Terminate all running LSP servers.
	pub fn terminate_all(&self) {
		let instances = {
			let mut state = self.state.lock().unwrap();
			state.projects.clear();
			state.pending_client_reqs.clear();
			state
				.servers
				.drain()
				.map(|(_, server)| server.instance)
				.collect::<Vec<_>>()
		};

		for instance in instances {
			{
				let mut current = instance.status.lock().unwrap();
				*current = xeno_broker_proto::types::LspServerStatus::Stopped;
			}
			tokio::spawn(async move {
				instance.terminate().await;
			});
		}
	}

	/// Retrieves a snapshot of the current broker state for debugging or testing.
	#[doc(hidden)]
	pub fn get_state(
		&self,
	) -> (
		HashSet<SessionId>,
		HashMap<ServerId, Vec<SessionId>>,
		HashMap<ProjectKey, ServerId>,
	) {
		let state = self.state.lock().unwrap();
		let sessions = state.sessions.keys().cloned().collect();
		let servers = state
			.servers
			.iter()
			.map(|(id, s)| (*id, s.attached.iter().cloned().collect()))
			.collect();
		let projects = state.projects.clone();
		(sessions, servers, projects)
	}

	/// Retrieves the communication handle for a specific LSP server.
	pub fn get_server_tx(&self, server_id: ServerId) -> Option<LspTx> {
		let state = self.state.lock().unwrap();
		state
			.servers
			.get(&server_id)
			.map(|s| s.instance.lsp_tx.clone())
	}

	/// Send an asynchronous event to a registered session.
	pub fn send_event(&self, session_id: SessionId, event: IpcFrame) {
		let state = self.state.lock().unwrap();
		if let Some(session) = state.sessions.get(&session_id) {
			let _ = session.sink.send(MainLoopEvent::Outgoing(event));
		}
	}

	/// Broadcast an event to all sessions attached to a server.
	pub fn broadcast_to_server(&self, server_id: ServerId, event: Event) {
		let (sinks, frame) = {
			let state = self.state.lock().unwrap();
			let Some(server) = state.servers.get(&server_id) else {
				return;
			};

			let sinks: Vec<_> = server
				.attached
				.iter()
				.filter_map(|sid| state.sessions.get(sid).map(|s| s.sink.clone()))
				.collect();

			(sinks, IpcFrame::Event(event))
		};

		for sink in sinks {
			let _ = sink.send(MainLoopEvent::Outgoing(frame.clone()));
		}
	}

	/// Send an event only to the leader session of a server.
	pub fn send_to_leader(&self, server_id: ServerId, event: Event) {
		let (sink, frame) = {
			let state = self.state.lock().unwrap();
			let Some(server) = state.servers.get(&server_id) else {
				return;
			};

			let sink = state.sessions.get(&server.leader).map(|s| s.sink.clone());
			(sink, IpcFrame::Event(event))
		};

		if let Some(sink) = sink {
			let _ = sink.send(MainLoopEvent::Outgoing(frame));
		}
	}

	/// Register a pending server-to-editor request.
	pub fn register_client_request(
		&self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		tx: oneshot::Sender<LspReplyResult>,
	) -> Option<SessionId> {
		let mut state = self.state.lock().unwrap();
		let server = state.servers.get(&server_id)?;
		if server.attached.is_empty() {
			return None;
		}
		let leader = server.leader;

		state.pending_client_reqs.insert(
			(server_id, request_id),
			PendingClientReq {
				responder: leader,
				tx,
			},
		);

		Some(leader)
	}

	/// Complete a pending server-to-editor request with a reply.
	pub fn complete_client_request(
		&self,
		session_id: SessionId,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		result: LspReplyResult,
	) -> bool {
		let mut state = self.state.lock().unwrap();
		let Some(req) = state
			.pending_client_reqs
			.get(&(server_id, request_id.clone()))
		else {
			return false;
		};
		if req.responder != session_id {
			return false;
		}

		if let Some(req) = state.pending_client_reqs.remove(&(server_id, request_id)) {
			let _ = req.tx.send(result);
			true
		} else {
			false
		}
	}

	/// Update server status and notify all attached sessions.
	pub fn set_server_status(
		&self,
		server_id: ServerId,
		status: xeno_broker_proto::types::LspServerStatus,
	) {
		let (sessions, changed) = {
			let state = self.state.lock().unwrap();
			if let Some(server) = state.servers.get(&server_id) {
				let mut current = server.instance.status.lock().unwrap();
				if *current != status {
					*current = status;
					(server.attached.clone(), true)
				} else {
					(HashSet::new(), false)
				}
			} else {
				(HashSet::new(), false)
			}
		};

		if changed {
			let event = Event::LspStatus { server_id, status };
			for sid in sessions {
				self.send_event(sid, IpcFrame::Event(event.clone()));
			}
		}
	}

	/// Allocate a globally unique server ID.
	pub fn next_server_id(&self) -> ServerId {
		ServerId(self.next_server_id.fetch_add(1, Ordering::Relaxed))
	}

	/// Retrieve the DocId and last known version for a URI.
	pub fn get_doc_by_uri(&self, server_id: ServerId, uri: &str) -> Option<(DocId, u32)> {
		let state = self.state.lock().unwrap();
		let server = state.servers.get(&server_id)?;
		server.docs.by_uri.get(uri).cloned()
	}

	/// Update document version tracking by observing editor-to-server traffic.
	pub fn on_editor_message(&self, server_id: ServerId, msg: &xeno_lsp::Message) {
		if let xeno_lsp::Message::Notification(notif) = msg {
			let mut state = self.state.lock().unwrap();
			if let Some(server) = state.servers.get_mut(&server_id) {
				match notif.method.as_str() {
					"textDocument/didOpen" | "textDocument/didChange" => {
						if let Some(doc) = notif.params.get("textDocument")
							&& let Some(uri) = doc.get("uri").and_then(|u| u.as_str())
						{
							let version =
								doc.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
							server.docs.update(uri.to_string(), version);
						}
					}
					_ => {}
				}
			}
		}
	}
}

/// Handle to a child process that can be real or mocked for tests.
#[derive(Debug)]
pub enum ChildHandle {
	/// Real spawned process.
	Real(tokio::process::Child),
	/// Mock handle for tests.
	#[doc(hidden)]
	Mock,
}

/// A running LSP server instance and its associated handles.
#[derive(Debug)]
pub struct LspInstance {
	/// Socket for sending requests/notifications to the server's stdio.
	pub lsp_tx: LspTx,
	/// Child process handle for lifecycle management.
	pub child: ChildHandle,
	/// Synchronized server lifecycle status.
	pub status: Mutex<xeno_broker_proto::types::LspServerStatus>,
}

impl LspInstance {
	/// Create a new LspInstance with a real child process.
	pub fn new(
		lsp_tx: LspTx,
		child: tokio::process::Child,
		status: xeno_broker_proto::types::LspServerStatus,
	) -> Self {
		Self {
			lsp_tx,
			child: ChildHandle::Real(child),
			status: Mutex::new(status),
		}
	}

	/// Create a mock LspInstance for tests (no real process).
	#[doc(hidden)]
	pub fn mock(lsp_tx: LspTx, status: xeno_broker_proto::types::LspServerStatus) -> Self {
		Self {
			lsp_tx,
			child: ChildHandle::Mock,
			status: Mutex::new(status),
		}
	}

	/// Best-effort graceful shutdown (shutdown request + exit notif), then kill if needed.
	pub async fn terminate(self) {
		let mut child = match self.child {
			ChildHandle::Mock => return,
			ChildHandle::Real(child) => child,
		};

		// 1) shutdown request (best-effort)
		let shutdown_req: xeno_lsp::AnyRequest = serde_json::from_value(serde_json::json!({
			"id": 0,
			"method": "shutdown",
			"params": serde_json::Value::Null
		}))
		.unwrap();

		let (tx, rx) = oneshot::channel::<xeno_lsp::AnyResponse>();
		let _ = self
			.lsp_tx
			.send(MainLoopEvent::OutgoingRequest(shutdown_req, tx));
		let _ = tokio::time::timeout(Duration::from_millis(300), rx).await;

		// 2) exit notification (best-effort)
		let exit_notif: xeno_lsp::AnyNotification = serde_json::from_value(serde_json::json!({
			"method": "exit",
			"params": serde_json::Value::Null
		}))
		.unwrap();

		let _ = self
			.lsp_tx
			.send(MainLoopEvent::Outgoing(xeno_lsp::Message::Notification(
				exit_notif,
			)));

		// 3) Wait briefly for natural exit, then kill.
		let exited = tokio::time::timeout(Duration::from_millis(500), child.wait()).await;
		if exited.is_ok() {
			return;
		}

		let _ = child.kill().await;
		let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
	}
}

/// Registry for document version tracking.
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

#[cfg(test)]
mod tests;
