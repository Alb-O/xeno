//! Peer communication sockets for Language Servers and Clients.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, oneshot};

use crate::event::AnyEvent;
use crate::message::Message;
use crate::types::{AnyNotification, AnyRequest, AnyResponse, RequestId};
use crate::{Error, Result};

/// Internal event types for the main loop.
#[derive(Debug)]
pub(crate) enum MainLoopEvent {
	/// A message to send to the peer.
	Outgoing(Message),
	/// A message to send to the peer with an acknowledgement.
	OutgoingWithBarrier(Message, oneshot::Sender<()>),
	/// An outgoing request with response channel.
	OutgoingRequest(AnyRequest, oneshot::Sender<AnyResponse>),
	/// A user-defined loopback event.
	Any(AnyEvent),
}

/// Macro to implement common socket wrapper methods for Client/Server sockets.
macro_rules! impl_socket_wrapper {
	($name:ident) => {
		impl $name {
			/// Create a closed socket outside a main loop. Any interaction will immediately return
			/// an error of [`Error::ServiceStopped`].
			///
			/// This works as a placeholder where a socket is required but actually unused.
			///
			/// # Note
			///
			/// To prevent accidental misusages, this method is NOT implemented as
			/// [`Default::default`] intentionally.
			#[must_use]
			pub fn new_closed() -> Self {
				Self(PeerSocket::new_closed())
			}

			/// Send a request to the peer and wait for its response.
			///
			/// # Errors
			/// - [`Error::ServiceStopped`] when the service main loop stopped.
			/// - [`Error::Response`] when the peer replies an error.
			pub async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result> {
				self.0.request::<R>(params).await
			}

			/// Send a notification to the peer and wait for its response.
			///
			/// This is done asynchronously. An `Ok` result indicates the message is successfully
			/// queued, but may not be sent to the peer yet.
			///
			/// # Errors
			/// - [`Error::ServiceStopped`] when the service main loop stopped.
			pub fn notify<N: Notification>(&self, params: N::Params) -> Result<()> {
				self.0.notify::<N>(params)
			}

			/// Emit an arbitrary loopback event object to the service handler.
			///
			/// This is done asynchronously. An `Ok` result indicates the message is successfully
			/// queued, but may not be processed yet.
			///
			/// # Errors
			/// - [`Error::ServiceStopped`] when the service main loop stopped.
			pub fn emit<E: Send + 'static>(&self, event: E) -> Result<()> {
				self.0.emit::<E>(event)
			}
		}
	};
}

/// The socket for Language Server to communicate with the Language Client peer.
#[derive(Debug, Clone)]
pub struct ClientSocket(pub(crate) PeerSocket);
impl_socket_wrapper!(ClientSocket);

/// The socket for Language Client to communicate with the Language Server peer.
#[derive(Debug, Clone)]
pub struct ServerSocket(pub(crate) PeerSocket);
impl_socket_wrapper!(ServerSocket);

/// Internal socket for communicating with the peer.
#[derive(Debug, Clone)]
pub(crate) struct PeerSocket {
	/// Channel sender for outgoing messages.
	pub tx: mpsc::UnboundedSender<MainLoopEvent>,
}

impl PeerSocket {
	/// Creates a closed socket that always returns errors.
	pub fn new_closed() -> Self {
		let (tx, _rx) = mpsc::unbounded_channel();
		Self { tx }
	}

	/// Sends an event to the main loop.
	pub(crate) fn send(&self, v: MainLoopEvent) -> Result<()> {
		self.tx.send(v).map_err(|_| Error::ServiceStopped)
	}

	/// Sends a typed request and returns a future for the response.
	pub fn request<R: Request>(&self, params: R::Params) -> PeerSocketRequestFuture<R::Result> {
		let req = AnyRequest {
			id: RequestId::Number(0),
			method: R::METHOD.into(),
			params: serde_json::to_value(params).expect("Failed to serialize"),
		};
		let (tx, rx) = oneshot::channel();
		// If this fails, the oneshot channel will also be closed, and it is handled by
		// `PeerSocketRequestFuture`.
		let _: Result<_, _> = self.send(MainLoopEvent::OutgoingRequest(req, tx));
		PeerSocketRequestFuture {
			rx,
			_marker: PhantomData,
		}
	}

	/// Sends a typed notification to the peer.
	pub fn notify<N: Notification>(&self, params: N::Params) -> Result<()> {
		let notif = AnyNotification {
			method: N::METHOD.into(),
			params: serde_json::to_value(params).expect("Failed to serialize"),
		};
		self.send(MainLoopEvent::Outgoing(Message::Notification(notif)))
	}

	/// Emits a user-defined event to the service handler.
	pub fn emit<E: Send + 'static>(&self, event: E) -> Result<()> {
		self.send(MainLoopEvent::Any(AnyEvent::new(event)))
	}
}

/// Future for awaiting a response to a peer request.
pub(crate) struct PeerSocketRequestFuture<T> {
	/// Channel receiver for the response.
	rx: oneshot::Receiver<AnyResponse>,
	/// Marker for the expected result type.
	_marker: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> Future for PeerSocketRequestFuture<T> {
	type Output = Result<T>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let resp = ready!(
			Pin::new(&mut self.rx)
				.poll(cx)
				.map_err(|_| Error::ServiceStopped)
		)?;
		Poll::Ready(match resp.error {
			None => Ok(serde_json::from_value(resp.result.unwrap_or_default())?),
			Some(err) => Err(Error::Response(err)),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn closed_client_socket() {
		let socket = ClientSocket::new_closed();
		assert!(matches!(
			socket.notify::<lsp_types::notification::Exit>(()),
			Err(Error::ServiceStopped)
		));
		assert!(matches!(
			socket.request::<lsp_types::request::Shutdown>(()).await,
			Err(Error::ServiceStopped)
		));
		assert!(matches!(socket.emit(42i32), Err(Error::ServiceStopped)));
	}

	#[tokio::test]
	async fn closed_server_socket() {
		let socket = ServerSocket::new_closed();
		assert!(matches!(
			socket.notify::<lsp_types::notification::Exit>(()),
			Err(Error::ServiceStopped)
		));
		assert!(matches!(
			socket.request::<lsp_types::request::Shutdown>(()).await,
			Err(Error::ServiceStopped)
		));
		assert!(matches!(socket.emit(42i32), Err(Error::ServiceStopped)));
	}
}
