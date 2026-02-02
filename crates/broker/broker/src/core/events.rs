//! Event broadcasting to sessions.
//!
//! Methods for sending events to individual sessions, server-attached sessions,
//! or leader sessions.

use std::sync::Arc;

use xeno_broker_proto::types::{Event, IpcFrame, ServerId, SessionId};
use xeno_rpc::MainLoopEvent;

use super::BrokerCore;

impl BrokerCore {
	/// Send an asynchronous event to a registered session.
	///
	/// Returns false if the send failed, indicating the session is dead.
	pub fn send_event(&self, session_id: SessionId, event: IpcFrame) -> bool {
		let sink = {
			let routing = self.routing.lock().unwrap();
			routing.sessions.get(&session_id).map(|s| s.sink.clone())
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
			let routing = self.routing.lock().unwrap();
			let Some(server) = routing.servers.get(&server_id) else {
				return;
			};

			let session_sinks: Vec<(SessionId, super::SessionSink)> = server
				.attached
				.iter()
				.filter_map(|sid| routing.sessions.get(sid).map(|s| (*sid, s.sink.clone())))
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
			let routing = self.routing.lock().unwrap();
			let Some(server) = routing.servers.get(&server_id) else {
				return;
			};

			let leader_id = server.leader;
			let sink = routing.sessions.get(&leader_id).map(|s| s.sink.clone());
			(leader_id, sink, IpcFrame::Event(event))
		};

		if let Some(sink) = sink
			&& sink.send(MainLoopEvent::Outgoing(frame)).is_err()
		{
			let core = self.clone();
			tokio::spawn(async move {
				core.handle_session_send_failure(leader_id);
			});
		}
	}
}
