//! Session management service.

use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};
use xeno_broker_proto::types::{IpcFrame, SessionId};
use xeno_rpc::MainLoopEvent;

use crate::core::SessionSink;

/// Commands for the session service actor.
#[derive(Debug)]
pub enum SessionCmd {
	/// Register a new editor session connection.
	Register {
		/// The session identity.
		sid: SessionId,
		/// The outbound communication sink.
		sink: SessionSink,
	},
	/// Unregister a session, typically on explicit disconnect.
	Unregister {
		/// The session identity.
		sid: SessionId,
	},
	/// Send an IPC frame to a specific session.
	Send {
		/// Target session.
		sid: SessionId,
		/// Payload to transmit.
		frame: IpcFrame,
	},
	/// Send an IPC frame and verify successful delivery.
	SendChecked {
		/// Target session.
		sid: SessionId,
		/// Payload to transmit.
		frame: IpcFrame,
		/// Channel for reporting delivery success/failure.
		reply: oneshot::Sender<bool>,
	},
	/// Broadcast a frame to multiple sessions.
	Broadcast {
		/// List of target session identities.
		sids: Vec<SessionId>,
		/// Payload to transmit.
		frame: IpcFrame,
		/// Optional session to exclude from the broadcast (e.g. the sender).
		exclude: Option<SessionId>,
	},
}

/// Handle for communicating with the `SessionService`.
#[derive(Clone, Debug)]
pub struct SessionHandle {
	tx: mpsc::Sender<SessionCmd>,
}

impl SessionHandle {
	/// Wraps a command sender in a typed handle.
	pub fn new(tx: mpsc::Sender<SessionCmd>) -> Self {
		Self { tx }
	}

	/// Registers a session sink.
	pub async fn register(&self, sid: SessionId, sink: SessionSink) {
		let _ = self.tx.send(SessionCmd::Register { sid, sink }).await;
	}

	/// Unregisters a session and triggers cleanup fan-out.
	pub async fn unregister(&self, sid: SessionId) {
		let _ = self.tx.send(SessionCmd::Unregister { sid }).await;
	}

	/// Sends an event best-effort.
	pub async fn send(&self, sid: SessionId, frame: IpcFrame) {
		let _ = self.tx.send(SessionCmd::Send { sid, frame }).await;
	}

	/// Sends an event and awaits confirmation of delivery to the local sink.
	///
	/// Returns false if the sink is missing or the send operation failed.
	pub async fn send_checked(&self, sid: SessionId, frame: IpcFrame) -> bool {
		let (reply_tx, reply_rx) = oneshot::channel();
		if self
			.tx
			.send(SessionCmd::SendChecked {
				sid,
				frame,
				reply: reply_tx,
			})
			.await
			.is_err()
		{
			return false;
		}
		reply_rx.await.unwrap_or(false)
	}

	/// Broadcasts a frame to multiple sessions.
	pub async fn broadcast(
		&self,
		sids: Vec<SessionId>,
		frame: IpcFrame,
		exclude: Option<SessionId>,
	) {
		let _ = self
			.tx
			.send(SessionCmd::Broadcast {
				sids,
				frame,
				exclude,
			})
			.await;
	}
}

/// Actor service that owns all connected editor sinks.
///
/// `SessionService` is the single source of truth for IPC delivery and send-failure
/// detection. When a send operation fails, this service triggers non-blocking
/// cleanup fan-out to other services (routing, shared state, etc.) to detach the dead session.
pub struct SessionService {
	rx: mpsc::Receiver<SessionCmd>,
	sessions: HashMap<SessionId, SessionSink>,
	routing: Option<super::routing::RoutingHandle>,
	shared_state: Option<super::shared_state::SharedStateHandle>,
}

impl SessionService {
	/// Spawns the session service actor task.
	///
	/// Returns the public handle and two "handshake" channels for injecting
	/// other service handles to resolve circular dependencies.
	pub fn start() -> (
		SessionHandle,
		mpsc::Sender<super::routing::RoutingHandle>,
		mpsc::Sender<super::shared_state::SharedStateHandle>,
	) {
		let (tx, rx) = mpsc::channel(256);
		let (routing_tx, routing_rx) = mpsc::channel(1);
		let (shared_tx, shared_rx) = mpsc::channel(1);

		let service = Self {
			rx,
			sessions: HashMap::new(),
			routing: None,
			shared_state: None,
		};

		tokio::spawn(service.run(routing_rx, shared_rx));

		(SessionHandle::new(tx), routing_tx, shared_tx)
	}

	async fn run(
		mut self,
		mut routing_rx: mpsc::Receiver<super::routing::RoutingHandle>,
		mut shared_rx: mpsc::Receiver<super::shared_state::SharedStateHandle>,
	) {
		if let Some(h) = routing_rx.recv().await {
			self.routing = Some(h);
		}
		if let Some(h) = shared_rx.recv().await {
			self.shared_state = Some(h);
		}

		while let Some(cmd) = self.rx.recv().await {
			match cmd {
				SessionCmd::Register { sid, sink } => {
					self.sessions.insert(sid, sink);
				}
				SessionCmd::Unregister { sid } => {
					self.sessions.remove(&sid);
					self.spawn_session_cleanup(sid);
				}
				SessionCmd::Send { sid, frame } => {
					if !self.do_send(sid, frame) {
						self.spawn_session_cleanup(sid);
					}
				}
				SessionCmd::SendChecked { sid, frame, reply } => {
					let success = self.do_send(sid, frame);
					if !success {
						self.spawn_session_cleanup(sid);
					}
					let _ = reply.send(success);
				}
				SessionCmd::Broadcast {
					sids,
					frame,
					exclude,
				} => {
					let mut failed = Vec::new();
					for sid in sids {
						if Some(sid) != exclude && !self.do_send(sid, frame.clone()) {
							failed.push(sid);
						}
					}
					for sid in failed {
						self.spawn_session_cleanup(sid);
					}
				}
			}
		}
	}

	fn do_send(&mut self, sid: SessionId, frame: IpcFrame) -> bool {
		let Some(sink) = self.sessions.get(&sid) else {
			return false;
		};
		if sink.send(MainLoopEvent::Outgoing(frame)).is_ok() {
			return true;
		}
		self.sessions.remove(&sid);
		false
	}

	fn spawn_session_cleanup(&self, sid: SessionId) {
		if let Some(routing) = self.routing.clone() {
			tokio::spawn(async move {
				routing.session_lost(sid).await;
			});
		}
		if let Some(shared_state) = self.shared_state.clone() {
			tokio::spawn(async move {
				shared_state.session_lost(sid).await;
			});
		}
	}
}
