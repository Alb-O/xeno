use std::future::Future;
use std::ops::ControlFlow;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::sync::oneshot;
use tower_service::Service;

use crate::event::AnyEvent;
use crate::message::Message;
use crate::socket::{MainLoopEvent, PeerSocket};
use crate::types::{AnyNotification, AnyRequest, AnyResponse, ErrorCode, ResponseError};
use crate::{ClientSocket, LspService, Result, ServerSocket};

/// Future for awaiting a response forwarded through a peer socket.
pub struct PeerSocketResponseFuture {
	/// Channel receiver for the response.
	rx: oneshot::Receiver<AnyResponse>,
}

impl Future for PeerSocketResponseFuture {
	type Output = Result<serde_json::Value, ResponseError>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		match Pin::new(&mut self.rx).poll(cx) {
			Poll::Ready(Ok(resp)) => Poll::Ready(match resp.error {
				None => Ok(resp.result.unwrap_or_default()),
				Some(resp_err) => Err(resp_err),
			}),
			Poll::Ready(Err(_closed)) => Poll::Ready(Err(ResponseError::new(ErrorCode::INTERNAL_ERROR, "forwarding service stopped"))),
			Poll::Pending => Poll::Pending,
		}
	}
}

impl PeerSocket {
	/// Forwards a request and returns a future for the response.
	fn on_call(&mut self, req: AnyRequest) -> PeerSocketResponseFuture {
		let (tx, rx) = oneshot::channel();
		let _: Result<_, _> = self.send(MainLoopEvent::OutgoingRequest(req, tx));
		PeerSocketResponseFuture { rx }
	}

	/// Forwards a notification to the peer.
	fn on_notify(&mut self, notif: AnyNotification) -> ControlFlow<Result<()>> {
		match self.send(MainLoopEvent::Outgoing(Message::Notification(notif))) {
			Ok(()) => ControlFlow::Continue(()),
			Err(err) => ControlFlow::Break(Err(err)),
		}
	}

	/// Forwards a custom event to the peer.
	fn on_emit(&mut self, event: AnyEvent) -> ControlFlow<Result<()>> {
		match self.send(MainLoopEvent::Any(event)) {
			Ok(()) => ControlFlow::Continue(()),
			Err(err) => ControlFlow::Break(Err(err)),
		}
	}
}

/// Generates Service and LspService implementations for socket types.
macro_rules! define_socket_wrappers {
    ($($ty:ty),*) => {
        $(
        impl Service<AnyRequest> for $ty {
            type Response = serde_json::Value;
            type Error = ResponseError;
            type Future = PeerSocketResponseFuture;

            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                // `*Socket` has an unbounded buffer, thus is always ready.
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, req: AnyRequest) -> Self::Future {
                self.0.on_call(req)
            }
        }

        impl LspService for $ty {
            fn notify(&mut self, notif: AnyNotification) -> ControlFlow<Result<()>> {
                self.0.on_notify(notif)
            }

            fn emit(&mut self, event: AnyEvent) -> ControlFlow<Result<()>> {
                self.0.on_emit(event)
            }
        }
        )*
    };
}

define_socket_wrappers!(ClientSocket, ServerSocket);
