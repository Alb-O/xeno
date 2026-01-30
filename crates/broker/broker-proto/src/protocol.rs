//! Broker protocol implementation using xeno_rpc.

use std::io::{Error as IoError, ErrorKind};

use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use xeno_rpc::{CounterIdGen, Inbound, Protocol};

use crate::types::{ErrorCode, Event, IpcFrame, Request, RequestId, Response, ResponsePayload};

/// Broker protocol implementation using length-delimited postcard encoding.
#[derive(Debug, Clone, Default)]
pub struct BrokerProtocol;

impl BrokerProtocol {
	/// Creates a new broker protocol instance.
	#[must_use]
	pub const fn new() -> Self {
		Self
	}
}

impl Protocol for BrokerProtocol {
	type Id = RequestId;
	type Message = IpcFrame;
	type Request = Request;
	type Response = Response;
	type Notification = Event;
	type ReqResult = ResponsePayload;
	type ReqError = ErrorCode;
	type LoopError = IoError;
	type IdGen = CounterIdGen;

	fn next_id(id_gen: &mut Self::IdGen) -> Self::Id {
		RequestId(id_gen.next())
	}

	async fn read_message(
		&mut self,
		input: &mut (impl AsyncBufRead + Unpin + Send),
	) -> std::io::Result<Self::Message> {
		// Read length prefix (4 bytes, little endian)
		let mut len_bytes = [0u8; 4];
		input.read_exact(&mut len_bytes).await?;
		let len = u32::from_le_bytes(len_bytes) as usize;

		// Sanity check: max 16MB message
		if len > 16 * 1024 * 1024 {
			return Err(IoError::new(
				ErrorKind::InvalidData,
				format!("message too large: {} bytes", len),
			));
		}

		// Read message bytes
		let mut buf = vec![0u8; len];
		input.read_exact(&mut buf).await?;

		// Decode with postcard
		postcard::from_bytes(&buf).map_err(|e| IoError::new(ErrorKind::InvalidData, e.to_string()))
	}

	async fn write_message(
		&mut self,
		output: &mut (impl AsyncWrite + Unpin + Send),
		msg: &Self::Message,
	) -> std::io::Result<()> {
		// Encode with postcard
		let buf = postcard::to_allocvec(msg)
			.map_err(|e| IoError::new(ErrorKind::InvalidData, e.to_string()))?;

		// Sanity check
		if buf.len() > 16 * 1024 * 1024 {
			return Err(IoError::new(
				ErrorKind::InvalidData,
				format!("message too large: {} bytes", buf.len()),
			));
		}

		// Write length prefix (4 bytes, little endian)
		let len_bytes = (buf.len() as u32).to_le_bytes();
		output.write_all(&len_bytes).await?;

		// Write message bytes
		output.write_all(&buf).await?;
		output.flush().await?;

		Ok(())
	}

	fn split_inbound(
		msg: Self::Message,
	) -> Inbound<Self::Request, Self::Response, Self::Notification> {
		match msg {
			IpcFrame::Request(req) => Inbound::Request(req),
			IpcFrame::Response(resp) => Inbound::Response(resp),
			IpcFrame::Event(event) => Inbound::Notification(event),
		}
	}

	fn request_id(req: &Self::Request) -> Self::Id {
		req.id
	}

	fn set_request_id(req: &mut Self::Request, id: Self::Id) {
		req.id = id;
	}

	fn response_id(resp: &Self::Response) -> Self::Id {
		resp.request_id
	}

	fn wrap_request(req: Self::Request) -> Self::Message {
		IpcFrame::Request(req)
	}

	fn wrap_response(resp: Self::Response) -> Self::Message {
		IpcFrame::Response(resp)
	}

	fn wrap_notification(notif: Self::Notification) -> Self::Message {
		IpcFrame::Event(notif)
	}

	fn response_ok(id: Self::Id, result: Self::ReqResult) -> Self::Response {
		Response {
			request_id: id,
			payload: Some(result),
			error: None,
		}
	}

	fn response_err(id: Self::Id, error: Self::ReqError) -> Self::Response {
		Response {
			request_id: id,
			payload: None,
			error: Some(error),
		}
	}

	fn post_response_messages(resp: &Self::Response) -> Vec<Self::Message> {
		match &resp.payload {
			Some(ResponsePayload::Subscribed) => vec![IpcFrame::Event(Event::Heartbeat)],
			_ => Vec::new(),
		}
	}

	fn is_disconnect(err: &Self::LoopError) -> bool {
		matches!(
			err.kind(),
			ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe | ErrorKind::ConnectionReset
		)
	}
}
