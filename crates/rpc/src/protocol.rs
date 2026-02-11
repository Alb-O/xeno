//! Protocol abstraction for wire formats and message semantics.

use std::hash::Hash;

use tokio::io::{AsyncBufRead, AsyncWrite};

/// Simple counter-based ID generator for protocols.
///
/// This is the standard implementation for sequential integer IDs.
#[derive(Debug, Default, Clone, Copy)]
pub struct CounterIdGen(pub u64);

impl CounterIdGen {
	/// Creates a new counter starting at 0.
	#[must_use]
	pub const fn new() -> Self {
		Self(0)
	}

	/// Generates the next unique ID and increments the counter.
	#[allow(clippy::should_implement_trait, reason = "convention")]
	pub fn next(&mut self) -> u64 {
		let id = self.0;
		self.0 += 1;
		id
	}
}

/// Classification of an inbound message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inbound<Req, Resp, Notif> {
	/// An incoming request.
	Request(Req),
	/// An incoming response.
	Response(Resp),
	/// An incoming notification.
	Notification(Notif),
}

/// Protocol binding between the generic pump and a concrete message format.
pub trait Protocol: Send + 'static {
	/// Identifier type for requests/responses.
	type Id: Eq + Hash + Clone + Send + 'static;

	/// The wire message type.
	type Message: Send + 'static;

	/// Request message type.
	type Request: Send + 'static;

	/// Response message type.
	type Response: Send + 'static;

	/// Notification message type.
	type Notification: Send + 'static;

	/// Successful result type from request handlers.
	type ReqResult: Send + 'static;

	/// Error type from request handlers.
	type ReqError: Send + 'static;

	/// Error type for IO/codec/protocol violations in the loop.
	type LoopError: From<std::io::Error> + Send + 'static;

	/// State for generating unique request IDs.
	type IdGen: Send + 'static;

	/// Generate the next unique request ID.
	fn next_id(id_gen: &mut Self::IdGen) -> Self::Id;

	/// Read a complete message from the input stream.
	fn read_message(
		&mut self,
		input: &mut (impl AsyncBufRead + Unpin + Send),
	) -> impl std::future::Future<Output = std::result::Result<Self::Message, Self::LoopError>> + Send;

	/// Write a message to the output stream.
	fn write_message(
		&mut self,
		output: &mut (impl AsyncWrite + Unpin + Send),
		msg: &Self::Message,
	) -> impl std::future::Future<Output = std::result::Result<(), Self::LoopError>> + Send;

	/// Classify an inbound message.
	fn split_inbound(msg: Self::Message) -> Inbound<Self::Request, Self::Response, Self::Notification>;

	/// Get the ID from a request.
	fn request_id(req: &Self::Request) -> Self::Id;

	/// Set the ID on a request.
	fn set_request_id(req: &mut Self::Request, id: Self::Id);

	/// Get the ID from a response.
	fn response_id(resp: &Self::Response) -> Self::Id;

	/// Wrap a request into a wire message.
	fn wrap_request(req: Self::Request) -> Self::Message;

	/// Wrap a response into a wire message.
	fn wrap_response(resp: Self::Response) -> Self::Message;

	/// Wrap a notification into a wire message.
	fn wrap_notification(notif: Self::Notification) -> Self::Message;

	/// Returns true if the loop should assign a new ID from `id_gen`.
	///
	/// Defaults to `true`. Protocols that support pre-assigned IDs
	/// can override this to skip ID generation.
	fn should_assign_id(req: &Self::Request) -> bool {
		let _ = req;
		true
	}

	/// Create a successful response message.
	fn response_ok(id: Self::Id, result: Self::ReqResult) -> Self::Response;

	/// Create an error response.
	fn response_err(id: Self::Id, error: Self::ReqError) -> Self::Response;

	/// Returns true if the loop error represents a clean disconnect.
	fn is_disconnect(_err: &Self::LoopError) -> bool {
		false
	}

	/// Additional messages to emit immediately after a successful response.
	fn post_response_messages(_resp: &Self::Response) -> Vec<Self::Message> {
		Vec::new()
	}
}
