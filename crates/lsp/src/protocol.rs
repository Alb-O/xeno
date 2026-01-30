//! JSON-RPC protocol implementation for xeno-rpc.

use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufRead, AsyncWrite};

use crate::message::Message;
use crate::types::{AnyNotification, AnyRequest, AnyResponse, RequestId};
use crate::{Error, Result};

/// JSON-RPC protocol implementation.
#[derive(Debug, Clone)]
pub struct JsonRpcProtocol;

impl JsonRpcProtocol {
	/// Creates a new JSON-RPC protocol instance.
	#[must_use]
	pub const fn new() -> Self {
		Self
	}
}

impl Default for JsonRpcProtocol {
	fn default() -> Self {
		Self::new()
	}
}

impl xeno_rpc::Protocol for JsonRpcProtocol {
	type Id = RequestId;
	type Message = Message;
	type Request = AnyRequest;
	type Response = AnyResponse;
	type Notification = AnyNotification;
	type ReqResult = JsonValue;
	type ReqError = crate::types::ResponseError;
	type LoopError = Error;
	type IdGen = xeno_rpc::CounterIdGen;

	fn next_id(id_gen: &mut Self::IdGen) -> Self::Id {
		RequestId::Number(id_gen.next() as i32)
	}

	async fn read_message(
		&mut self,
		input: &mut (impl AsyncBufRead + Unpin + Send),
	) -> Result<Self::Message> {
		Message::read(input).await
	}

	async fn write_message(
		&mut self,
		output: &mut (impl AsyncWrite + Unpin + Send),
		msg: &Self::Message,
	) -> Result<()> {
		msg.write(output).await
	}

	fn split_inbound(
		msg: Self::Message,
	) -> xeno_rpc::Inbound<Self::Request, Self::Response, Self::Notification> {
		match msg {
			Message::Request(req) => xeno_rpc::Inbound::Request(req),
			Message::Response(resp) => xeno_rpc::Inbound::Response(resp),
			Message::Notification(notif) => xeno_rpc::Inbound::Notification(notif),
		}
	}

	fn request_id(req: &Self::Request) -> Self::Id {
		req.id.clone()
	}

	fn set_request_id(req: &mut Self::Request, id: Self::Id) {
		req.id = id;
	}

	fn response_id(resp: &Self::Response) -> Self::Id {
		resp.id.clone()
	}

	fn wrap_request(req: Self::Request) -> Self::Message {
		Message::Request(req)
	}

	fn wrap_response(resp: Self::Response) -> Self::Message {
		Message::Response(resp)
	}

	fn wrap_notification(notif: Self::Notification) -> Self::Message {
		Message::Notification(notif)
	}

	fn response_ok(id: Self::Id, result: Self::ReqResult) -> Self::Response {
		AnyResponse {
			id,
			result: Some(result),
			error: None,
		}
	}

	fn response_err(id: Self::Id, error: Self::ReqError) -> Self::Response {
		AnyResponse {
			id,
			result: None,
			error: Some(error),
		}
	}
}

/// Request ID generator for JSON-RPC.
#[derive(Debug, Default)]
pub struct IdGen {
	counter: i32,
}

impl IdGen {
	/// Creates a new ID generator starting at 0.
	#[must_use]
	pub const fn new() -> Self {
		Self { counter: 0 }
	}

	/// Generates the next unique request ID.
	#[allow(clippy::should_implement_trait, reason = "convention")]
	pub fn next(&mut self) -> RequestId {
		let id = RequestId::Number(self.counter);
		self.counter += 1;
		id
	}
}
