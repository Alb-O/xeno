//! Internal socket for communicating between user code and the main loop.

use tokio::sync::{mpsc, oneshot};

use crate::error::{Error, Result};
use crate::event::AnyEvent;

/// Internal event types for the main loop.
#[derive(Debug)]
pub enum MainLoopEvent<M, Req, Resp> {
	/// A message to send to the peer.
	Outgoing(M),
	/// A message to send to the peer with an acknowledgement.
	OutgoingWithBarrier(M, oneshot::Sender<()>),
	/// An outgoing request with response channel.
	OutgoingRequest(Req, oneshot::Sender<Resp>),
	/// A user-defined loopback event.
	Any(AnyEvent),
}

/// Internal socket for communicating with the peer.
#[derive(Debug)]
pub struct PeerSocket<M, Req, Resp> {
	/// Channel sender for outgoing messages.
	pub(crate) tx: mpsc::UnboundedSender<MainLoopEvent<M, Req, Resp>>,
}

impl<M, Req, Resp> Clone for PeerSocket<M, Req, Resp> {
	fn clone(&self) -> Self {
		Self {
			tx: self.tx.clone(),
		}
	}
}

impl<M, Req, Resp> PeerSocket<M, Req, Resp> {
	/// Creates a closed socket that always returns errors.
	pub fn new_closed() -> Self {
		let (tx, _rx) = mpsc::unbounded_channel();
		Self { tx }
	}

	/// Creates a socket from an existing sender.
	///
	/// This is intended for testing purposes in dependent crates.
	#[doc(hidden)]
	pub fn from_sender(tx: mpsc::UnboundedSender<MainLoopEvent<M, Req, Resp>>) -> Self {
		Self { tx }
	}

	/// Sends an event to the main loop.
	pub fn send(&self, v: MainLoopEvent<M, Req, Resp>) -> Result<()> {
		self.tx.send(v).map_err(|_| Error::Stopped)
	}

	/// Emits a user-defined event to the service handler.
	pub fn emit<E: Send + 'static>(&self, event: E) -> Result<()> {
		self.send(MainLoopEvent::Any(AnyEvent::new(event)))
	}
}

impl<M, Req, Resp> Default for PeerSocket<M, Req, Resp> {
	fn default() -> Self {
		Self::new_closed()
	}
}
