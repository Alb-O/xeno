//! Outbound message queue and dispatcher for LSP client.

use futures::channel::oneshot;
use tokio::sync::{mpsc, watch};

use crate::message::Message;
use crate::socket::{MainLoopEvent, ServerSocket};
use crate::types::{AnyNotification, AnyRequest, AnyResponse};

use super::state::ServerState;

/// Outbound queue capacity.
pub(super) const OUTBOUND_QUEUE_LEN: usize = 256;

/// Write barrier for confirming socket write completion.
pub(super) type WriteBarrier = oneshot::Sender<()>;

/// Messages sent from ClientHandle to the outbound dispatcher.
pub(super) enum OutboundMsg {
	Notification {
		notification: AnyNotification,
		barrier: Option<WriteBarrier>,
	},
	Request {
		request: AnyRequest,
		response_tx: oneshot::Sender<AnyResponse>,
	},
}

/// Send a message to the socket.
fn send_msg(socket: &ServerSocket, msg: OutboundMsg) -> std::result::Result<(), ()> {
	match msg {
		OutboundMsg::Notification {
			notification,
			barrier,
		} => match barrier {
			Some(barrier) => socket
				.0
				.send(MainLoopEvent::OutgoingWithAck(
					Message::Notification(notification),
					barrier,
				))
				.map_err(|_| ()),
			None => socket
				.0
				.send(MainLoopEvent::Outgoing(Message::Notification(notification)))
				.map_err(|_| ()),
		},
		OutboundMsg::Request {
			request,
			response_tx,
		} => socket
			.0
			.send(MainLoopEvent::OutgoingRequest(request, response_tx))
			.map_err(|_| ()),
	}
}

/// Dispatches outbound messages to the server.
///
/// Forwards messages to the socket until the channel closes or server dies.
/// Callers gate sends via [`ClientHandle::wait_ready`] before the server is ready.
pub(super) async fn outbound_dispatcher(
	mut rx: mpsc::Receiver<OutboundMsg>,
	socket: ServerSocket,
	mut state_rx: watch::Receiver<ServerState>,
) {
	loop {
		tokio::select! {
			biased;

			msg = rx.recv() => {
				let Some(msg) = msg else { break };
				let _ = send_msg(&socket, msg);
			}

			_ = state_rx.changed() => {
				if *state_rx.borrow() == ServerState::Dead {
					break;
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use futures::StreamExt;

	use super::*;
	use crate::socket::PeerSocket;

	#[tokio::test]
	async fn outbound_dispatcher_forwards_notifications() {
		let (peer_tx, mut peer_rx) = futures::channel::mpsc::unbounded();
		let socket = ServerSocket(PeerSocket { tx: peer_tx });

		let (_state_tx, state_rx) = watch::channel(ServerState::Ready);
		let (outbound_tx, outbound_rx) = mpsc::channel(4);
		tokio::spawn(outbound_dispatcher(outbound_rx, socket, state_rx));

		outbound_tx
			.send(OutboundMsg::Notification {
				notification: AnyNotification {
					method: "test".into(),
					params: serde_json::Value::Null,
				},
				barrier: None,
			})
			.await
			.unwrap();

		let event = peer_rx.next().await.expect("event");
		match event {
			MainLoopEvent::Outgoing(Message::Notification(notif)) => {
				assert_eq!(notif.method, "test");
			}
			other => panic!("unexpected event: {:?}", other),
		}
	}

	#[tokio::test]
	async fn outbound_dispatcher_fires_write_barrier() {
		let (peer_tx, mut peer_rx) = futures::channel::mpsc::unbounded();
		let socket = ServerSocket(PeerSocket { tx: peer_tx });

		let (_state_tx, state_rx) = watch::channel(ServerState::Ready);
		let (outbound_tx, outbound_rx) = mpsc::channel(4);
		tokio::spawn(outbound_dispatcher(outbound_rx, socket, state_rx));

		let (barrier_tx, barrier_rx) = oneshot::channel();
		outbound_tx
			.send(OutboundMsg::Notification {
				notification: AnyNotification {
					method: "test".into(),
					params: serde_json::Value::Null,
				},
				barrier: Some(barrier_tx),
			})
			.await
			.unwrap();

		// Consume the OutgoingWithAck event and fire the barrier (simulating mainloop)
		let event = peer_rx.next().await.expect("event");
		match event {
			MainLoopEvent::OutgoingWithAck(Message::Notification(notif), barrier) => {
				assert_eq!(notif.method, "test");
				let _ = barrier.send(());
			}
			other => panic!("unexpected event: {:?}", other),
		}

		// Barrier should complete
		assert!(barrier_rx.await.is_ok());
	}

	#[tokio::test]
	async fn outbound_dispatcher_preserves_fifo_order() {
		let (peer_tx, mut peer_rx) = futures::channel::mpsc::unbounded();
		let socket = ServerSocket(PeerSocket { tx: peer_tx });

		let (_state_tx, state_rx) = watch::channel(ServerState::Ready);
		let (outbound_tx, outbound_rx) = mpsc::channel(4);
		tokio::spawn(outbound_dispatcher(outbound_rx, socket, state_rx));

		outbound_tx
			.send(OutboundMsg::Notification {
				notification: AnyNotification {
					method: "first".into(),
					params: serde_json::Value::Null,
				},
				barrier: None,
			})
			.await
			.unwrap();
		outbound_tx
			.send(OutboundMsg::Notification {
				notification: AnyNotification {
					method: "second".into(),
					params: serde_json::Value::Null,
				},
				barrier: None,
			})
			.await
			.unwrap();

		let first = peer_rx.next().await.expect("first event");
		let second = peer_rx.next().await.expect("second event");

		match first {
			MainLoopEvent::Outgoing(Message::Notification(notif)) => {
				assert_eq!(notif.method, "first");
			}
			other => panic!("unexpected first event: {:?}", other),
		}

		match second {
			MainLoopEvent::Outgoing(Message::Notification(notif)) => {
				assert_eq!(notif.method, "second");
			}
			other => panic!("unexpected second event: {:?}", other),
		}
	}

	#[tokio::test]
	async fn outbound_dispatcher_stops_on_dead() {
		let (peer_tx, _peer_rx) = futures::channel::mpsc::unbounded();
		let socket = ServerSocket(PeerSocket { tx: peer_tx });

		let (state_tx, state_rx) = watch::channel(ServerState::Starting);
		let (_outbound_tx, outbound_rx) = mpsc::channel(4);
		let handle = tokio::spawn(outbound_dispatcher(outbound_rx, socket, state_rx));

		state_tx.send(ServerState::Dead).unwrap();

		// Task should exit
		tokio::time::timeout(std::time::Duration::from_millis(100), handle)
			.await
			.expect("dispatcher should exit on Dead state")
			.unwrap();
	}
}
