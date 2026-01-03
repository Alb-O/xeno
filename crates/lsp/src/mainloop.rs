//! Service main loop driver for Language Servers and Clients.

use std::collections::HashMap;
use std::future::{Future, poll_fn};
use std::ops::ControlFlow;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use futures::channel::{mpsc, oneshot};
use futures::io::BufReader;
use futures::stream::FuturesUnordered;
use futures::{
	AsyncBufRead, AsyncRead, AsyncWrite, FutureExt, SinkExt, StreamExt, pin_mut, select_biased,
};
use pin_project_lite::pin_project;
use serde_json::Value as JsonValue;

use crate::message::Message;
use crate::socket::{ClientSocket, MainLoopEvent, PeerSocket, ServerSocket};
use crate::types::{AnyResponse, RequestId, ResponseError};
use crate::{LspService, Result};

/// Macro to define getter methods for accessing inner service fields.
#[macro_export]
macro_rules! define_getters {
    (impl[$($generic:tt)*] $ty:ty, $field:ident : $field_ty:ty) => {
        impl<$($generic)*> $ty {
            /// Get a reference to the inner service.
            #[must_use]
            pub fn get_ref(&self) -> &$field_ty {
                &self.$field
            }

            /// Get a mutable reference to the inner service.
            #[must_use]
            pub fn get_mut(&mut self) -> &mut $field_ty {
                &mut self.$field
            }

            /// Consume self, returning the inner service.
            #[must_use]
            pub fn into_inner(self) -> $field_ty {
                self.$field
            }
        }
    };
}

/// Service main loop driver for either Language Servers or Language Clients.
pub struct MainLoop<S: LspService> {
	/// The wrapped LSP service.
	service: S,
	/// Receiver for internal events from sockets.
	rx: mpsc::UnboundedReceiver<MainLoopEvent>,
	/// Counter for generating outgoing request IDs.
	outgoing_id: i32,
	/// Pending outgoing requests awaiting responses.
	outgoing: HashMap<RequestId, oneshot::Sender<AnyResponse>>,
	/// Concurrent request handlers in flight.
	tasks: FuturesUnordered<RequestFuture<S::Future>>,
}

define_getters!(impl[S: LspService] MainLoop<S>, service: S);

impl<S> MainLoop<S>
where
	S: LspService<Response = JsonValue>,
	ResponseError: From<S::Error>,
{
	/// Create a Language Server main loop.
	#[must_use]
	pub fn new_server(builder: impl FnOnce(ClientSocket) -> S) -> (Self, ClientSocket) {
		let (this, socket) = Self::new(|socket| builder(ClientSocket(socket)));
		(this, ClientSocket(socket))
	}

	/// Create a Language Client main loop.
	#[must_use]
	pub fn new_client(builder: impl FnOnce(ServerSocket) -> S) -> (Self, ServerSocket) {
		let (this, socket) = Self::new(|socket| builder(ServerSocket(socket)));
		(this, ServerSocket(socket))
	}

	/// Internal constructor for creating a main loop with a peer socket.
	fn new(builder: impl FnOnce(PeerSocket) -> S) -> (Self, PeerSocket) {
		let (tx, rx) = mpsc::unbounded();
		let socket = PeerSocket { tx };
		let this = Self {
			service: builder(socket.clone()),
			rx,
			outgoing_id: 0,
			outgoing: HashMap::new(),
			tasks: FuturesUnordered::new(),
		};
		(this, socket)
	}

	/// Drive the service main loop to provide the service.
	///
	/// Shortcut to [`MainLoop::run`] that accept an `impl AsyncRead` and implicit wrap it in a
	/// [`BufReader`].
	#[allow(clippy::missing_errors_doc, reason = "errors documented in Self::run")]
	pub async fn run_buffered(self, input: impl AsyncRead, output: impl AsyncWrite) -> Result<()> {
		self.run(BufReader::new(input), output).await
	}

	/// Drive the service main loop to provide the service.
	///
	/// # Errors
	///
	/// - `Error::Io` when the underlying `input` or `output` raises an error.
	/// - `Error::Deserialize` when the peer sends undecodable or invalid message.
	/// - `Error::Protocol` when the peer violates Language Server Protocol.
	/// - Other errors raised from service handlers.
	pub async fn run(mut self, input: impl AsyncBufRead, output: impl AsyncWrite) -> Result<()> {
		pin_mut!(input, output);
		let incoming = futures::stream::unfold(input, |mut input| async move {
			Some((Message::read(&mut input).await, input))
		});
		let outgoing = futures::sink::unfold(output, |mut output, msg| async move {
			Message::write(&msg, &mut output).await.map(|()| output)
		});
		pin_mut!(incoming, outgoing);

		let mut flush_fut = futures::future::Fuse::terminated();
		let ret = loop {
			// Outgoing > internal > incoming.
			// Preference on outgoing data provides back pressure in case of
			// flooding incoming requests.
			let ctl = select_biased! {
				// Concurrently flush out the previous message.
				ret = flush_fut => { ret?; continue; }

				resp = self.tasks.select_next_some() => ControlFlow::Continue(Some(Message::Response(resp))),
				event = self.rx.next() => self.dispatch_event(event.expect("Sender is alive")),
				msg = incoming.next() => {
					let dispatch_fut = self.dispatch_message(msg.expect("Never ends")?).fuse();
					pin_mut!(dispatch_fut);
					// NB. Concurrently wait for `poll_ready`, and write out the last message.
					// If the service is waiting for client's response of the last request, while
					// the last message is not delivered on the first write, it can deadlock.
					loop {
						select_biased! {
							// Dispatch first. It usually succeeds immediately for non-requests,
							// and the service is hardly busy.
							ctl = dispatch_fut => break ctl,
							ret = flush_fut => { ret?; continue }
						}
					}
				}
			};
			let msg = match ctl {
				ControlFlow::Continue(Some(msg)) => msg,
				ControlFlow::Continue(None) => continue,
				ControlFlow::Break(ret) => break ret,
			};
			// Flush the previous one and load a new message to send.
			outgoing.feed(msg).await?;
			flush_fut = outgoing.flush().fuse();
		};

		let flush_ret = outgoing.close().await;
		ret.and(flush_ret)
	}

	/// Routes an incoming message to the appropriate handler.
	async fn dispatch_message(&mut self, msg: Message) -> ControlFlow<Result<()>, Option<Message>> {
		match msg {
			Message::Request(req) => {
				if let Err(err) = poll_fn(|cx| self.service.poll_ready(cx)).await {
					let resp = AnyResponse {
						id: req.id,
						result: None,
						error: Some(err.into()),
					};
					return ControlFlow::Continue(Some(Message::Response(resp)));
				}
				let id = req.id.clone();
				let fut = self.service.call(req);
				self.tasks.push(RequestFuture { fut, id: Some(id) });
			}
			Message::Response(resp) => {
				if let Some(resp_tx) = self.outgoing.remove(&resp.id) {
					// The result may be ignored.
					let _: Result<_, _> = resp_tx.send(resp);
				}
			}
			Message::Notification(notif) => {
				self.service.notify(notif)?;
			}
		}
		ControlFlow::Continue(None)
	}

	/// Routes an internal event (outgoing message or user event).
	fn dispatch_event(&mut self, event: MainLoopEvent) -> ControlFlow<Result<()>, Option<Message>> {
		match event {
			MainLoopEvent::OutgoingRequest(mut req, resp_tx) => {
				req.id = RequestId::Number(self.outgoing_id);
				assert!(self.outgoing.insert(req.id.clone(), resp_tx).is_none());
				self.outgoing_id += 1;
				ControlFlow::Continue(Some(Message::Request(req)))
			}
			MainLoopEvent::Outgoing(msg) => ControlFlow::Continue(Some(msg)),
			MainLoopEvent::Any(event) => {
				self.service.emit(event)?;
				ControlFlow::Continue(None)
			}
		}
	}
}

pin_project! {
	struct RequestFuture<Fut> {
		#[pin]
		fut: Fut,
		id: Option<RequestId>,
	}
}

impl<Fut, Error> Future for RequestFuture<Fut>
where
	Fut: Future<Output = Result<JsonValue, Error>>,
	ResponseError: From<Error>,
{
	type Output = AnyResponse;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();
		let (mut result, mut error) = (None, None);
		match ready!(this.fut.poll(cx)) {
			Ok(v) => result = Some(v),
			Err(err) => error = Some(err.into()),
		}
		Poll::Ready(AnyResponse {
			id: this.id.take().expect("Future is consumed"),
			result,
			error,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Error;

	fn _main_loop_future_is_send<S>(
		f: MainLoop<S>,
		input: impl AsyncBufRead + Send,
		output: impl AsyncWrite + Send,
	) -> impl Send
	where
		S: LspService<Response = JsonValue> + Send,
		S::Future: Send,
		S::Error: From<Error> + Send,
		ResponseError: From<S::Error>,
	{
		f.run(input, output)
	}
}
