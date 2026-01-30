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
	/// Pending server->client requests awaiting an editor reply (leader-routed).
	pending_s2c: HashMap<(ServerId, xeno_lsp::RequestId), PendingS2cReq>,
	/// Pending client->server requests awaiting an LSP server response (broker rewrites ids).
	pending_c2s: HashMap<(ServerId, xeno_lsp::RequestId), PendingC2sReq>,
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
		// Use a sentinel value for missing cwd to prevent incorrect deduplication.
		// Configs without cwd should not dedup with each other.
		let cwd = cfg.cwd.clone().unwrap_or_else(|| {
			// Use a unique sentinel based on command+args hash to ensure
			// different configs without cwd don't collide
			format!("__NO_CWD_{:016x}__", compute_config_hash(cfg))
		});
		Self {
			command: cfg.command.clone(),
			args: cfg.args.clone(),
			cwd,
		}
	}
}

/// Compute a stable hash of config for the no-cwd sentinel.
fn compute_config_hash(cfg: &LspServerConfig) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};
	let mut hasher = DefaultHasher::new();
	cfg.command.hash(&mut hasher);
	cfg.args.hash(&mut hasher);
	hasher.finish()
}

#[derive(Debug)]
struct SessionEntry {
	sink: SessionSink,
	attached: HashSet<ServerId>,
}

/// State for a single LSP server instance managed by the broker.
#[derive(Debug)]
struct ServerEntry {
	/// Handle to the running LSP process and its control channels.
	instance: LspInstance,
	/// Identity of the server (cmd/args/cwd) used for deduplication.
	project: ProjectKey,
	/// Set of editor sessions currently attached to this server.
	attached: HashSet<SessionId>,
	/// The session responsible for answering server-initiated requests.
	leader: SessionId,
	/// Tracked documents and their versions for this server.
	docs: DocRegistry,
	/// Generation token used to invalidate stale lease tasks.
	lease_gen: u64,
	/// Ownership registry for text sync gating (single-writer per URI).
	doc_owners: DocOwnerRegistry,
	/// Monotonic allocator for broker "wire ids" for requests sent to the LSP server.
	next_wire_req_id: u64,
}

/// Metadata for a pending server-to-client request awaiting a reply from the editor.
#[derive(Debug)]
struct PendingS2cReq {
	/// The session elected as leader at the time of the request.
	responder: SessionId,
	/// Completion channel for the proxied LSP response.
	tx: oneshot::Sender<LspReplyResult>,
}

/// Metadata for a pending client-to-server request awaiting a response from the LSP server.
#[derive(Debug)]
pub struct PendingC2sReq {
	/// The editor session that initiated the request.
	pub origin_session: SessionId,
	/// The original request id as seen by the editor (pre-rewrite).
	pub origin_id: xeno_lsp::RequestId,
}

/// Snapshot of the current broker state for debugging or testing.
pub type BrokerStateSnapshot = (
	HashSet<SessionId>,
	HashMap<ServerId, Vec<SessionId>>,
	HashMap<ProjectKey, ServerId>,
);

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
	///
	/// # Arguments
	/// * `session_id` - Unique identifier for the editor session.
	/// * `sink` - Communication channel back to the editor.
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
	///
	/// This method performs authoritative cleanup of the session state. It detaches
	/// the session from all running servers, potentially triggering idle leases,
	/// and cancels any pending requests where this session was the responder.
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

		self.cancel_all_client_requests_for_session(session_id);
	}

	/// Look up an existing server for a project configuration.
	///
	/// Returns the [`ServerId`] if a matching server is already running or warm.
	pub fn find_server_for_project(&self, config: &LspServerConfig) -> Option<ServerId> {
		let key = ProjectKey::from(config);
		let state = self.state.lock().unwrap();
		state.projects.get(&key).cloned()
	}

	/// Attach an editor session to an existing LSP server.
	///
	/// If the session is the first to attach, it is elected as the leader.
	/// Attaching invalidates any pending idle leases for the server.
	pub fn attach_session(&self, server_id: ServerId, session_id: SessionId) -> bool {
		let mut state = self.state.lock().unwrap();
		let Some(server) = state.servers.get_mut(&server_id) else {
			return false;
		};

		server.attached.insert(session_id);
		if server.attached.len() == 1 {
			server.leader = session_id;
		}

		server.lease_gen += 1;

		if let Some(session) = state.sessions.get_mut(&session_id) {
			session.attached.insert(server_id);
		}

		true
	}

	/// Detach a session from an LSP server.
	///
	/// If the detached session was the leader, a new leader is elected.
	/// If this was the last session, an idle lease is scheduled to eventually
	/// terminate the server process.
	pub fn detach_session(self: &Arc<Self>, server_id: ServerId, session_id: SessionId) {
		let mut maybe_schedule: Option<(u64, tokio::time::Instant)> = None;
		let mut leader_changed = false;
		let mut old_leader = session_id;

		{
			let mut state = self.state.lock().unwrap();

			if let Some(server) = state.servers.get_mut(&server_id) {
				server.attached.remove(&session_id);

				if server.leader == session_id {
					old_leader = session_id;
					if let Some(&new_leader) = server.attached.iter().next() {
						server.leader = new_leader;
						leader_changed = true;
					}
				}

				if server.attached.is_empty() {
					server.lease_gen += 1;
					let generation = server.lease_gen;
					let deadline = tokio::time::Instant::now() + self.config.idle_lease;
					maybe_schedule = Some((generation, deadline));
				}
			}

			if let Some(session) = state.sessions.get_mut(&session_id) {
				session.attached.remove(&server_id);
			}
		}

		if leader_changed {
			tracing::info!(
				?server_id,
				?old_leader,
				"Leader changed, cancelling pending requests"
			);
			self.cancel_pending_for_leader_change(server_id, old_leader);
		}

		if let Some((generation, deadline)) = maybe_schedule {
			let core = self.clone();
			tokio::spawn(async move {
				tokio::time::sleep_until(deadline).await;
				core.check_lease_expiry(server_id, generation);
			});
		}
	}

	fn cancel_pending_for_leader_change(&self, server_id: ServerId, old_leader: SessionId) {
		let to_cancel: Vec<xeno_lsp::RequestId> = {
			let state = self.state.lock().unwrap();
			state
				.pending_s2c
				.iter()
				.filter(|((sid, _), req)| *sid == server_id && req.responder == old_leader)
				.map(|((_, rid), _)| rid.clone())
				.collect()
		};
		for request_id in to_cancel {
			self.cancel_client_request(server_id, request_id);
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

			let has_inflight = state.pending_s2c.keys().any(|(sid, _)| *sid == server_id)
				|| state.pending_c2s.keys().any(|(sid, _)| *sid == server_id);
			if has_inflight {
				return;
			}

			let server = state.servers.remove(&server_id).unwrap();
			state.projects.remove(&server.project);

			state.pending_s2c.retain(|(sid, _), _| *sid != server_id);
			state.pending_c2s.retain(|(sid, _), _| *sid != server_id);

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
				doc_owners: DocOwnerRegistry::default(),
				next_wire_req_id: 1,
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

			state.pending_s2c.retain(|(sid, _), _| *sid != server_id);

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

	/// Terminate all running LSP servers and clear all attachments.
	pub fn terminate_all(&self) {
		let instances = {
			let mut state = self.state.lock().unwrap();
			state.projects.clear();
			state.pending_s2c.clear();
			state.pending_c2s.clear();

			for session in state.sessions.values_mut() {
				session.attached.clear();
			}

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
	pub fn get_state(&self) -> BrokerStateSnapshot {
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
	///
	/// Returns false if the send failed, indicating the session is dead.
	pub fn send_event(&self, session_id: SessionId, event: IpcFrame) -> bool {
		let sink = {
			let state = self.state.lock().unwrap();
			state.sessions.get(&session_id).map(|s| s.sink.clone())
		};
		if let Some(sink) = sink {
			sink.send(MainLoopEvent::Outgoing(event)).is_ok()
		} else {
			false
		}
	}

	/// Broadcast an event to all sessions attached to an LSP server.
	///
	/// Authoritatively cleans up any sessions where the IPC send fails.
	pub fn broadcast_to_server(self: &Arc<Self>, server_id: ServerId, event: Event) {
		let (session_sinks, frame) = {
			let state = self.state.lock().unwrap();
			let Some(server) = state.servers.get(&server_id) else {
				return;
			};

			let session_sinks: Vec<(SessionId, SessionSink)> = server
				.attached
				.iter()
				.filter_map(|sid| state.sessions.get(sid).map(|s| (*sid, s.sink.clone())))
				.collect();

			(session_sinks, IpcFrame::Event(event))
		};

		let mut failed_sessions = Vec::new();
		for (session_id, sink) in session_sinks {
			if sink.send(MainLoopEvent::Outgoing(frame.clone())).is_err() {
				failed_sessions.push(session_id);
			}
		}

		if !failed_sessions.is_empty() {
			let core = self.clone();
			tokio::spawn(async move {
				for session_id in failed_sessions {
					tracing::warn!(?session_id, "Broadcast send failed, triggering cleanup");
					core.handle_session_send_failure(session_id);
				}
			});
		}
	}

	/// Send an event to the leader session of an LSP server.
	///
	/// Authoritatively cleans up the leader session if the IPC send fails.
	pub fn send_to_leader(self: &Arc<Self>, server_id: ServerId, event: Event) {
		let (leader_id, sink, frame) = {
			let state = self.state.lock().unwrap();
			let Some(server) = state.servers.get(&server_id) else {
				return;
			};

			let leader_id = server.leader;
			let sink = state.sessions.get(&leader_id).map(|s| s.sink.clone());
			(leader_id, sink, IpcFrame::Event(event))
		};

		if let Some(sink) = sink
			&& sink.send(MainLoopEvent::Outgoing(frame)).is_err()
		{
			tracing::warn!(?leader_id, "Leader session send failed, triggering cleanup");
			let core = self.clone();
			tokio::spawn(async move {
				core.handle_session_send_failure(leader_id);
			});
		}
	}

	/// Register a pending server-to-editor request.
	///
	/// Returns the [`SessionId`] of the leader elected to respond.
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

		state.pending_s2c.insert(
			(server_id, request_id),
			PendingS2cReq {
				responder: leader,
				tx,
			},
		);

		Some(leader)
	}

	/// Complete a pending server-to-editor request with a reply.
	///
	/// Returns true if the request was successfully completed.
	pub fn complete_client_request(
		&self,
		session_id: SessionId,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
		result: LspReplyResult,
	) -> bool {
		let mut state = self.state.lock().unwrap();
		let Some(req) = state.pending_s2c.get(&(server_id, request_id.clone())) else {
			return false;
		};
		if req.responder != session_id {
			return false;
		}

		if let Some(req) = state.pending_s2c.remove(&(server_id, request_id)) {
			let _ = req.tx.send(result);
			true
		} else {
			false
		}
	}

	/// Cancel a pending server-to-editor request with a standard error response.
	pub fn cancel_client_request(
		&self,
		server_id: ServerId,
		request_id: xeno_lsp::RequestId,
	) -> bool {
		let mut state = self.state.lock().unwrap();
		if let Some(req) = state.pending_s2c.remove(&(server_id, request_id)) {
			let _ = req.tx.send(Err(xeno_lsp::ResponseError::new(
				xeno_lsp::ErrorCode::REQUEST_CANCELLED,
				"Request cancelled by broker",
			)));
			true
		} else {
			false
		}
	}

	/// Cancel all pending server-to-client requests for a given session.
	pub fn cancel_all_client_requests_for_session(&self, session_id: SessionId) {
		let to_cancel: Vec<(ServerId, xeno_lsp::RequestId)> = {
			let state = self.state.lock().unwrap();
			state
				.pending_s2c
				.iter()
				.filter(|(_, req)| req.responder == session_id)
				.map(|(key, _)| key.clone())
				.collect()
		};
		for (server_id, request_id) in to_cancel {
			self.cancel_client_request(server_id, request_id);
		}
	}

	/// Cancel all pending server-to-client requests for a given server.
	pub fn cancel_all_client_requests_for_server(&self, server_id: ServerId) {
		let to_cancel: Vec<xeno_lsp::RequestId> = {
			let state = self.state.lock().unwrap();
			state
				.pending_s2c
				.iter()
				.filter(|((sid, _), _)| *sid == server_id)
				.map(|((_, rid), _)| rid.clone())
				.collect()
		};
		for request_id in to_cancel {
			self.cancel_client_request(server_id, request_id);
		}
	}

	/// Allocate a unique wire request ID for a server connection.
	///
	/// These IDs are used when forwarding client requests to the actual LSP server.
	/// The broker maintains a mapping between these wire IDs and the original client IDs
	/// to ensure correct response routing in a multiplexed environment.
	pub fn alloc_wire_request_id(&self, server_id: ServerId) -> Option<xeno_lsp::RequestId> {
		let mut state = self.state.lock().unwrap();
		let server = state.servers.get_mut(&server_id)?;
		let wire_num = server.next_wire_req_id;
		server.next_wire_req_id += 1;
		Some(xeno_lsp::RequestId::Number(wire_num as i32))
	}

	/// Register a pending client-to-server request mapping.
	///
	/// # Arguments
	/// * `server_id` - The target LSP server.
	/// * `wire_id` - The ID used on the wire to the server.
	/// * `origin_session` - The editor session that initiated the request.
	/// * `origin_id` - The original request ID from the editor.
	pub fn register_c2s_pending(
		&self,
		server_id: ServerId,
		wire_id: xeno_lsp::RequestId,
		origin_session: SessionId,
		origin_id: xeno_lsp::RequestId,
	) {
		let mut state = self.state.lock().unwrap();
		state.pending_c2s.insert(
			(server_id, wire_id),
			PendingC2sReq {
				origin_session,
				origin_id,
			},
		);
	}

	/// Remove and return the pending client-to-server mapping for a server and wire ID.
	pub fn take_c2s_pending(
		&self,
		server_id: ServerId,
		wire_id: &xeno_lsp::RequestId,
	) -> Option<PendingC2sReq> {
		let mut state = self.state.lock().unwrap();
		state.pending_c2s.remove(&(server_id, wire_id.clone()))
	}

	/// Cancel a pending client-to-server request and return the origin information.
	pub fn cancel_c2s_pending(
		&self,
		server_id: ServerId,
		wire_id: &xeno_lsp::RequestId,
	) -> Option<PendingC2sReq> {
		self.take_c2s_pending(server_id, wire_id)
	}

	/// Authoritatively cleans up a session that is determined to be dead.
	pub fn handle_session_send_failure(self: &Arc<Self>, session_id: SessionId) {
		tracing::info!(
			?session_id,
			"Handling session send failure, unregistering session"
		);
		self.cancel_all_client_requests_for_session(session_id);
		self.unregister_session(session_id);
	}

	/// Authoritatively cleans up a server that has exited or crashed.
	pub fn handle_server_exit(self: &Arc<Self>, server_id: ServerId, crashed: bool) {
		tracing::info!(?server_id, crashed, "Handling server exit");

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

			let pending_to_cancel: Vec<xeno_lsp::RequestId> = state
				.pending_s2c
				.iter()
				.filter(|((sid, _), _)| *sid == server_id)
				.map(|((_, rid), _)| rid.clone())
				.collect();

			state.pending_c2s.retain(|(sid, _), _| *sid != server_id);
			drop(state);

			for request_id in pending_to_cancel {
				self.cancel_client_request(server_id, request_id);
			}

			Some((server.attached, server.instance))
		};

		if let Some((attached_sessions, instance)) = maybe_instance {
			let status = if crashed {
				xeno_broker_proto::types::LspServerStatus::Crashed
			} else {
				xeno_broker_proto::types::LspServerStatus::Stopped
			};

			{
				let mut current = instance.status.lock().unwrap();
				*current = status;
			}

			let event = Event::LspStatus { server_id, status };
			for session_id in attached_sessions {
				self.send_event(session_id, IpcFrame::Event(event.clone()));
			}

			tokio::spawn(async move {
				instance.terminate().await;
			});
		}
	}

	/// Update server status and notify all attached sessions.
	pub fn set_server_status(
		self: &Arc<Self>,
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
			let mut failed_sessions = Vec::new();
			for sid in sessions {
				if !self.send_event(sid, IpcFrame::Event(event.clone())) {
					failed_sessions.push(sid);
				}
			}
			if !failed_sessions.is_empty() {
				let core = self.clone();
				tokio::spawn(async move {
					for session_id in failed_sessions {
						tracing::warn!(
							?session_id,
							"Status notification send failed, triggering cleanup"
						);
						core.handle_session_send_failure(session_id);
					}
				});
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

	/// Observe editor-to-server traffic to update document version tracking.
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

	/// Gate a text synchronization notification to enforce single-writer ownership per URI.
	///
	/// This method ensures that only the session that first opened a document can send
	/// subsequent modifications or close notifications for that document. If multiple sessions
	/// have the same document open, only the "owner" session is permitted to synchronize
	/// its state with the underlying LSP server.
	///
	/// Returns `true` if the notification is permitted and should be forwarded to the server.
	pub fn gate_text_sync(
		&self,
		session_id: SessionId,
		server_id: ServerId,
		notif: &xeno_lsp::AnyNotification,
	) -> bool {
		let mut state = self.state.lock().unwrap();
		let Some(server) = state.servers.get_mut(&server_id) else {
			return false;
		};

		match notif.method.as_str() {
			"textDocument/didOpen" => {
				let Some(doc) = notif.params.get("textDocument") else {
					return false;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return false;
				};
				let version = doc.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

				match server.doc_owners.by_uri.get_mut(uri) {
					None => {
						// First open: become owner
						let mut refcounts = HashMap::new();
						refcounts.insert(session_id, 1);
						server.doc_owners.by_uri.insert(
							uri.to_string(),
							DocOwnerState {
								owner: session_id,
								open_refcounts: refcounts,
								last_version: version,
							},
						);
						true
					}
					Some(owner_state) => {
						// Increment refcount for this session
						let count = owner_state.open_refcounts.entry(session_id).or_insert(0);
						*count += 1;
						// Only forward if this session is the owner
						if session_id == owner_state.owner {
							owner_state.last_version = version;
							true
						} else {
							false
						}
					}
				}
			}
			"textDocument/didChange" => {
				let Some(doc) = notif.params.get("textDocument") else {
					return false;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return false;
				};
				let version = doc.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

				match server.doc_owners.by_uri.get_mut(uri) {
					None => false,
					Some(owner_state) => {
						if session_id == owner_state.owner {
							owner_state.last_version = version;
							true
						} else {
							false
						}
					}
				}
			}
			"textDocument/didClose" => {
				let Some(doc) = notif.params.get("textDocument") else {
					return false;
				};
				let Some(uri) = doc.get("uri").and_then(|u| u.as_str()) else {
					return false;
				};

				match server.doc_owners.by_uri.get_mut(uri) {
					None => false,
					Some(owner_state) => {
						// Decrement refcount
						if let Some(count) = owner_state.open_refcounts.get_mut(&session_id) {
							if *count > 0 {
								*count -= 1;
							}
							if *count == 0 {
								owner_state.open_refcounts.remove(&session_id);
							}
						}

						// Only forward close if owner and no more refs
						let should_forward = session_id == owner_state.owner
							&& owner_state
								.open_refcounts
								.get(&session_id)
								.copied()
								.unwrap_or(0) == 0;

						if should_forward {
							server.doc_owners.by_uri.remove(uri);
							true
						} else {
							false
						}
					}
				}
			}
			_ => true,
		}
	}
}

/// Handle to a child process that can be real or mocked for tests.
#[derive(Debug)]
#[non_exhaustive]
pub enum ChildHandle {
	/// Real spawned process.
	Real(tokio::process::Child),
}

/// Channels for controlling and monitoring a server instance.
#[derive(Debug)]
pub struct ServerControl {
	/// Channel to request graceful termination.
	pub term_tx: tokio::sync::oneshot::Sender<()>,
	/// Channel to await completion of termination.
	pub done_rx: tokio::sync::oneshot::Receiver<()>,
}

/// A running LSP server instance and its associated handles.
#[derive(Debug)]
pub struct LspInstance {
	/// Socket for sending requests/notifications to the server's stdio.
	pub lsp_tx: LspTx,
	/// Control channels for the server lifecycle monitor.
	pub control: Option<ServerControl>,
	/// Synchronized server lifecycle status.
	pub status: Mutex<xeno_broker_proto::types::LspServerStatus>,
}

impl LspInstance {
	/// Create a new LspInstance with control channels.
	pub fn new(
		lsp_tx: LspTx,
		control: ServerControl,
		status: xeno_broker_proto::types::LspServerStatus,
	) -> Self {
		Self {
			lsp_tx,
			control: Some(control),
			status: Mutex::new(status),
		}
	}

	/// Create a mock LspInstance for tests (no real process).
	#[doc(hidden)]
	pub fn mock(lsp_tx: LspTx, status: xeno_broker_proto::types::LspServerStatus) -> Self {
		Self {
			lsp_tx,
			control: None,
			status: Mutex::new(status),
		}
	}

	/// Best-effort graceful shutdown (shutdown request + exit notif), then kill if needed.
	pub async fn terminate(mut self) {
		let Some(control) = self.control.take() else {
			// Mock instance - nothing to do
			return;
		};

		// 1) Request termination via control channel
		let _ = control.term_tx.send(());

		// 2) Wait for the monitor to complete (with timeout)
		let _ = tokio::time::timeout(Duration::from_secs(2), control.done_rx).await;
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

/// Registry for document ownership tracking (single-writer per URI).
#[derive(Debug, Default)]
struct DocOwnerRegistry {
	/// Keyed by URI.
	by_uri: HashMap<String, DocOwnerState>,
}

#[derive(Debug)]
struct DocOwnerState {
	/// Session that owns this document (can send text sync).
	owner: SessionId,
	/// Per-session open refcount (session may open doc multiple times).
	open_refcounts: HashMap<SessionId, u32>,
	/// Last observed version from the owner.
	last_version: u32,
}

/// Result of gating a text sync notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocGateDecision {
	/// Allow: session is the owner.
	AllowAsOwner,
	/// Reject: session is not the owner.
	RejectNotOwner,
}

/// Result struct for document gating operations.
pub struct DocGateResult {
	/// The decision (allow/reject).
	pub decision: DocGateDecision,
	/// The URI being gated.
	pub uri: String,
	/// The kind of operation.
	pub kind: DocGateKind,
}

/// Kind of text sync operation.
pub enum DocGateKind {
	/// Document opened.
	DidOpen {
		/// New version.
		version: u32,
	},
	/// Document changed.
	DidChange {
		/// New version.
		version: u32,
	},
	/// Document closed.
	DidClose,
}

#[cfg(test)]
mod tests;
