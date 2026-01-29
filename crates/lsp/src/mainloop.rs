//! Service main loop driver for Language Servers and Clients.

use std::collections::HashMap;
use std::future::{Future, poll_fn};
use std::ops::ControlFlow;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::{Duration, Instant};

use pin_project_lite::pin_project;
use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};
use tracing::error;

use crate::message::Message;
use crate::socket::{ClientSocket, MainLoopEvent, PeerSocket, ServerSocket};
use crate::types::{AnyResponse, RequestId, ResponseError};
use crate::{LspService, Result};

const TASK_DRAIN_MAX: usize = 32;
const TASK_DRAIN_WINDOW: Duration = Duration::from_millis(2);

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
	tasks: tokio::task::JoinSet<AnyResponse>,
}

struct OutgoingMessage {
	message: Message,
	barrier: Option<oneshot::Sender<()>>,
}

define_getters!(impl[S: LspService] MainLoop<S>, service: S);

impl<S> MainLoop<S>
where
	S: LspService<Response = JsonValue>,
	S::Future: Send + 'static,
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

	/// Create an internal constructor for creating a main loop with a peer socket.
	fn new(builder: impl FnOnce(PeerSocket) -> S) -> (Self, PeerSocket) {
		let (tx, rx) = mpsc::unbounded_channel();
		let socket = PeerSocket { tx };
		let this = Self {
			service: builder(socket.clone()),
			rx,
			outgoing_id: 0,
			outgoing: HashMap::new(),
			tasks: tokio::task::JoinSet::new(),
		};
		(this, socket)
	}

	/// Drive the service main loop to provide the service.
	///
	/// Shortcut to [`MainLoop::run`] that accept an `impl AsyncRead` and implicit wrap it in a
	/// [`BufReader`].
	#[allow(clippy::missing_errors_doc, reason = "errors documented in Self::run")]
	pub async fn run_buffered(
		self,
		input: impl AsyncRead + Unpin,
		output: impl AsyncWrite + Unpin,
	) -> Result<()> {
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
	pub async fn run(
		mut self,
		mut input: impl AsyncBufRead + Unpin,
		mut output: impl AsyncWrite + Unpin,
	) -> Result<()> {
		let mut task_budget_remaining = TASK_DRAIN_MAX;
		let mut task_budget_completed = 0u64;
		let mut task_budget_deadline = Instant::now() + TASK_DRAIN_WINDOW;

		let ret = loop {
			let ctl = tokio::select! {
				biased;

				resp = self.tasks.join_next(), if !self.tasks.is_empty() => {
					match resp {
						Some(Ok(resp)) => ControlFlow::Continue(Some(OutgoingMessage {
							message: Message::Response(resp),
							barrier: None,
						})),
						Some(Err(e)) => {
							error!(error = %e, "LSP task panicked or was cancelled");
							ControlFlow::Continue(None)
						}
						None => ControlFlow::Continue(None),
					}
				}

				event = self.rx.recv() => match event {
					Some(e) => self.dispatch_event(e),
					None => break Ok(()),
				},

				msg = Message::read(&mut input) => {
					self.dispatch_message(msg?).await
				}
			};

			let msg = match ctl {
				ControlFlow::Continue(Some(msg)) => msg,
				ControlFlow::Continue(None) => continue,
				ControlFlow::Break(ret) => break ret,
			};

			match msg.message {
				Message::Response(_) => {
					task_budget_remaining = task_budget_remaining.saturating_sub(1);
					task_budget_completed += 1;
					if task_budget_remaining == 0 || Instant::now() >= task_budget_deadline {
						// In a real implementation we might want to yield or prioritize other things here
					}
				}
				_ => {
					if task_budget_completed > 0 || !self.tasks.is_empty() {
						tracing::debug!(
							completed = task_budget_completed,
							backlog = self.tasks.len(),
							budget_max = TASK_DRAIN_MAX,
							budget_ms = TASK_DRAIN_WINDOW.as_millis() as u64,
							"lsp.tasks.drain_budget"
						);
					}
					task_budget_remaining = TASK_DRAIN_MAX;
					task_budget_completed = 0;
					task_budget_deadline = Instant::now() + TASK_DRAIN_WINDOW;
				}
			}

			let message = msg.message;
			let barrier = msg.barrier;

			Message::write(&message, &mut output).await?;
			if let Some(b) = barrier {
				let _ = b.send(());
			}
		};

		output.shutdown().await?;
		ret
	}

	/// Routes an incoming message to the appropriate handler.
	async fn dispatch_message(
		&mut self,
		msg: Message,
	) -> ControlFlow<Result<()>, Option<OutgoingMessage>> {
		match msg {
			Message::Request(req) => {
				if let Err(err) = poll_fn(|cx| self.service.poll_ready(cx)).await {
					let resp = AnyResponse {
						id: req.id,
						result: None,
						error: Some(err.into()),
					};
					return ControlFlow::Continue(Some(OutgoingMessage {
						message: Message::Response(resp),
						barrier: None,
					}));
				}
				let id = req.id.clone();
				let fut = self.service.call(req);
				self.tasks.spawn(RequestFuture { fut, id: Some(id) });
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
	fn dispatch_event(
		&mut self,
		event: MainLoopEvent,
	) -> ControlFlow<Result<()>, Option<OutgoingMessage>> {
		match event {
			MainLoopEvent::OutgoingRequest(mut req, resp_tx) => {
				req.id = RequestId::Number(self.outgoing_id);
				assert!(self.outgoing.insert(req.id.clone(), resp_tx).is_none());
				self.outgoing_id += 1;
				ControlFlow::Continue(Some(OutgoingMessage {
					message: Message::Request(req),
					barrier: None,
				}))
			}
			MainLoopEvent::Outgoing(msg) => ControlFlow::Continue(Some(OutgoingMessage {
				message: msg,
				barrier: None,
			})),
			MainLoopEvent::OutgoingWithBarrier(msg, barrier) => {
				ControlFlow::Continue(Some(OutgoingMessage {
					message: msg,
					barrier: Some(barrier),
				}))
			}
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
		input: impl AsyncBufRead + Send + Unpin,
		output: impl AsyncWrite + Send + Unpin,
	) -> impl Send
	where
		S: LspService<Response = JsonValue> + Send,
		S::Future: Send + 'static,
		S::Error: From<Error> + Send,
		ResponseError: From<S::Error>,
	{
		f.run(input, output)
	}
}
