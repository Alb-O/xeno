//! Generic service main loop driver.

use std::collections::HashMap;
use std::future::{Future, poll_fn};
use std::ops::ControlFlow;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use pin_project_lite::pin_project;
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tower_service::Service;

use crate::event::AnyEvent;
use crate::protocol::{Inbound, Protocol};
use crate::socket::{MainLoopEvent, PeerSocket};

const TASK_DRAIN_MAX: usize = 32;
const TASK_DRAIN_WINDOW_MS: u64 = 2;

type PendingMessage<M> = (M, Vec<M>);

/// Service trait for RPC handlers.
///
/// This combines `tower::Service` for requests with handlers for notifications and events.
pub trait RpcService<P: Protocol>:
	Service<P::Request, Response = P::ReqResult, Error = P::ReqError>
{
	/// Error type for notification/event handling.
	type LoopError: From<std::io::Error> + From<P::LoopError> + Send;

	/// Handle an incoming notification.
	///
	/// Notifications are delivered in order and synchronously.
	/// The return value controls whether to continue or break the main loop.
	fn notify(
		&mut self,
		notif: P::Notification,
	) -> ControlFlow<std::result::Result<(), Self::LoopError>>;

	/// Handle an arbitrary loopback event.
	///
	/// Events are delivered in order and synchronously.
	/// The return value controls whether to continue or break the main loop.
	fn emit(&mut self, event: AnyEvent) -> ControlFlow<std::result::Result<(), Self::LoopError>>;
}

/// Service main loop driver for generic RPC protocols.
///
/// This is a generic message pump that:
/// - Reads inbound messages from the input stream
/// - Dispatches requests, responses, and notifications appropriately
/// - Manages concurrent request handlers
/// - Routes internal events and outgoing messages
pub struct MainLoop<S, P: Protocol> {
	/// The wrapped service.
	service: S,
	/// Receiver for internal events from sockets.
	rx: mpsc::UnboundedReceiver<MainLoopEvent<P::Message, P::Request, P::Response>>,
	/// Generator for unique request IDs.
	id_gen: P::IdGen,
	/// Pending outgoing requests awaiting responses.
	outgoing: HashMap<P::Id, oneshot::Sender<P::Response>>,
	/// Concurrent request handlers in flight.
	tasks: JoinSet<P::Response>,
	/// Protocol codec and message handling.
	protocol: P,
}

impl<S, P> MainLoop<S, P>
where
	P: Protocol,
	S: RpcService<P> + Send + 'static,
	S::Future: Send + 'static,
{
	/// Create a new main loop with the given service builder and protocol.
	///
	/// Returns the main loop and a peer socket for communication.
	pub fn new(
		builder: impl FnOnce(PeerSocket<P::Message, P::Request, P::Response>) -> S,
		protocol: P,
		id_gen: P::IdGen,
	) -> (Self, PeerSocket<P::Message, P::Request, P::Response>) {
		let (tx, rx) = mpsc::unbounded_channel();
		let socket = PeerSocket { tx };
		let this = Self {
			service: builder(socket.clone()),
			rx,
			id_gen,
			outgoing: HashMap::new(),
			tasks: JoinSet::new(),
			protocol,
		};
		(this, socket)
	}

	/// Drive the service main loop.
	///
	/// This runs until an error occurs, EOF is reached, or the service signals shutdown.
	pub async fn run(
		mut self,
		mut input: impl AsyncBufRead + Unpin + Send,
		mut output: impl AsyncWrite + Unpin + Send,
	) -> std::result::Result<(), S::LoopError> {
		let mut task_budget_remaining = TASK_DRAIN_MAX;
		let mut task_budget_completed = 0u64;
		let mut task_budget_deadline =
			tokio::time::Instant::now() + tokio::time::Duration::from_millis(TASK_DRAIN_WINDOW_MS);

		let ret = loop {
			let drain_active = !self.tasks.is_empty()
				&& task_budget_remaining > 0
				&& tokio::time::Instant::now() < task_budget_deadline;

			let (ctl, from_task) = if drain_active {
				tokio::select! {
				   biased;

				resp = self.tasks.join_next(), if !self.tasks.is_empty() => {
					self.handle_task_response(resp)
				}

				   event = self.rx.recv() => match event {
					   Some(e) => (self.dispatch_event(e), false),
					   None => break Ok(()),
				   },

				   msg = self.protocol.read_message(&mut input) => {
					   match msg {
						   Ok(msg) => (self.dispatch_message(msg).await, false),
						   Err(e) => break Err(S::LoopError::from(e)),
					   }
				   }
				}
			} else {
				tokio::select! {
				   event = self.rx.recv() => match event {
					   Some(e) => (self.dispatch_event(e), false),
					   None => break Ok(()),
				   },

				   msg = self.protocol.read_message(&mut input) => {
					   match msg {
						   Ok(msg) => (self.dispatch_message(msg).await, false),
						   Err(e) => break Err(S::LoopError::from(e)),
					   }
				   },

				resp = self.tasks.join_next(), if !self.tasks.is_empty() => {
					self.handle_task_response(resp)
				}
				}
			};

			let msg = match ctl {
				ControlFlow::Continue(Some(msg)) => msg,
				ControlFlow::Continue(None) => continue,
				ControlFlow::Break(ret) => break ret,
			};
			let (main_msg, extras) = msg;
			if from_task {
				let frame_count = 1 + extras.len();
				task_budget_remaining = task_budget_remaining.saturating_sub(frame_count);
				task_budget_completed += frame_count as u64;
			} else {
				if task_budget_completed > 0 || !self.tasks.is_empty() {
					tracing::debug!(
						completed = task_budget_completed,
						backlog = self.tasks.len(),
						budget_max = TASK_DRAIN_MAX,
						"rpc.tasks.drain_budget"
					);
				}
				task_budget_remaining = TASK_DRAIN_MAX;
				task_budget_completed = 0;
				task_budget_deadline = tokio::time::Instant::now()
					+ tokio::time::Duration::from_millis(TASK_DRAIN_WINDOW_MS);
			}

			self.protocol
				.write_message(&mut output, &main_msg)
				.await
				.map_err(S::LoopError::from)?;
			for extra in extras {
				self.protocol
					.write_message(&mut output, &extra)
					.await
					.map_err(S::LoopError::from)?;
			}
		};

		output.shutdown().await.map_err(S::LoopError::from)?;
		ret
	}

	fn handle_task_response(
		&self,
		resp: Option<Result<P::Response, tokio::task::JoinError>>,
	) -> (
		ControlFlow<std::result::Result<(), S::LoopError>, Option<PendingMessage<P::Message>>>,
		bool,
	) {
		match resp {
			Some(Ok(resp)) => {
				let extras = P::post_response_messages(&resp);
				let wrapped = P::wrap_response(resp);
				(ControlFlow::Continue(Some((wrapped, extras))), true)
			}
			Some(Err(e)) => {
				tracing::error!(error = %e, "RPC task panicked or was cancelled");
				(ControlFlow::Continue(None), true)
			}
			None => (ControlFlow::Continue(None), true),
		}
	}

	/// Routes an incoming message to the appropriate handler.
	async fn dispatch_message(
		&mut self,
		msg: P::Message,
	) -> ControlFlow<std::result::Result<(), S::LoopError>, Option<PendingMessage<P::Message>>> {
		match P::split_inbound(msg) {
			Inbound::Request(req) => {
				// Ensure service is ready
				if let Err(err) = poll_fn(|cx| self.service.poll_ready(cx)).await {
					let id = P::request_id(&req);
					let resp = P::response_err(id, err);
					return ControlFlow::Continue(Some((P::wrap_response(resp), Vec::new())));
				}

				let id = P::request_id(&req);
				let fut = self.service.call(req);
				self.tasks.spawn(RequestFuture::<_, P> {
					fut,
					id: Some(id),
					_phantom: std::marker::PhantomData,
				});
			}

			Inbound::Response(resp) => {
				let id = P::response_id(&resp);
				if let Some(resp_tx) = self.outgoing.remove(&id) {
					let _: Result<_, _> = resp_tx.send(resp);
				}
			}

			Inbound::Notification(notif) => {
				return match self.service.notify(notif) {
					ControlFlow::Continue(()) => ControlFlow::Continue(None),
					ControlFlow::Break(result) => ControlFlow::Break(result),
				};
			}
		}

		ControlFlow::Continue(None)
	}

	/// Routes an internal event (outgoing message or user event).
	fn dispatch_event(
		&mut self,
		event: MainLoopEvent<P::Message, P::Request, P::Response>,
	) -> ControlFlow<std::result::Result<(), S::LoopError>, Option<PendingMessage<P::Message>>> {
		match event {
			MainLoopEvent::OutgoingRequest(mut req, resp_tx) => {
				let id = P::next_id(&mut self.id_gen);
				P::set_request_id(&mut req, id.clone());
				assert!(self.outgoing.insert(id, resp_tx).is_none());
				ControlFlow::Continue(Some((P::wrap_request(req), Vec::new())))
			}

			MainLoopEvent::Outgoing(msg) => ControlFlow::Continue(Some((msg, Vec::new()))),

			MainLoopEvent::OutgoingWithBarrier(msg, _barrier) => {
				// TODO: properly track barrier through write
				ControlFlow::Continue(Some((msg, Vec::new())))
			}

			MainLoopEvent::Any(event) => match self.service.emit(event) {
				ControlFlow::Continue(()) => ControlFlow::Continue(None),
				ControlFlow::Break(result) => ControlFlow::Break(result),
			},
		}
	}
}

/// Future wrapper for request handlers that captures the request ID.
pin_project! {
	struct RequestFuture<Fut, P: Protocol> {
		#[pin]
		fut: Fut,
		id: Option<P::Id>,
		_phantom: std::marker::PhantomData<P>,
	}
}

impl<Fut, P, T, E> Future for RequestFuture<Fut, P>
where
	Fut: Future<Output = std::result::Result<T, E>>,
	P: Protocol<ReqResult = T, ReqError = E>,
{
	type Output = P::Response;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();
		let id = this.id.take().expect("Future polled after completion");
		match ready!(this.fut.poll(cx)) {
			Ok(result) => Poll::Ready(P::response_ok(id, result)),
			Err(error) => Poll::Ready(P::response_err(id, error)),
		}
	}
}
