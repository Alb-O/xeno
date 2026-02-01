//! Server lifecycle management.
//!
//! Methods for registering, attaching, detaching, and managing LSP server instances.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use xeno_broker_proto::types::{Event, IpcFrame, LspServerConfig, ServerId, SessionId};

use super::{BrokerCore, LspTx, ProjectKey, ServerEntry};

/// Handle to a child process.
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

	/// Create a mock LspInstance for tests.
	#[doc(hidden)]
	pub fn mock(lsp_tx: LspTx, status: xeno_broker_proto::types::LspServerStatus) -> Self {
		Self {
			lsp_tx,
			control: None,
			status: Mutex::new(status),
		}
	}

	/// Best-effort graceful shutdown, then kill if needed.
	pub async fn terminate(mut self) {
		let Some(control) = self.control.take() else {
			return;
		};

		let _ = control.term_tx.send(());
		let _ = tokio::time::timeout(Duration::from_secs(2), control.done_rx).await;
	}
}

impl BrokerCore {
	/// Look up an existing server for a project configuration.
	///
	/// Returns the [`ServerId`] if a matching server is already running or warm.
	#[must_use]
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

		// Maintain deterministic leader (min session id)
		let min_id = *server.attached.iter().min().unwrap();
		server.leader = min_id;

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
					if let Some(&new_leader) = server.attached.iter().min() {
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
			tracing::info!(?server_id, ?old_leader, "leader re-elected");
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

	pub(super) fn cancel_pending_for_leader_change(
		&self,
		server_id: ServerId,
		old_leader: SessionId,
	) {
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
	pub(super) fn check_lease_expiry(self: &Arc<Self>, server_id: ServerId, generation: u64) {
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
				docs: super::text_sync::DocRegistry::default(),
				lease_gen: 0,
				doc_owners: super::text_sync::DocOwnerRegistry::default(),
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

	/// Authoritatively cleans up a server that has exited or crashed.
	pub fn handle_server_exit(self: &Arc<Self>, server_id: ServerId, crashed: bool) {
		tracing::info!(?server_id, crashed, "handling server exit");

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
						core.handle_session_send_failure(session_id);
					}
				});
			}
		}
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
}
