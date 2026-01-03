//! JSON-RPC message framing and I/O.

use futures::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use serde::{Deserialize, Serialize};

use crate::types::{AnyNotification, AnyRequest, AnyResponse};
use crate::{Error, Result};

/// A JSON-RPC message with version header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RawMessage<T> {
	/// JSON-RPC version (always "2.0").
	jsonrpc: RpcVersion,
	/// The wrapped message content.
	#[serde(flatten)]
	pub inner: T,
}

impl<T> RawMessage<T> {
	/// Creates a new message with JSON-RPC 2.0 version.
	pub fn new(inner: T) -> Self {
		Self {
			jsonrpc: RpcVersion::V2,
			inner,
		}
	}
}

/// JSON-RPC protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RpcVersion {
	/// JSON-RPC version 2.0.
	#[serde(rename = "2.0")]
	V2,
}

/// A JSON-RPC message (request, response, or notification).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Message {
	/// An incoming or outgoing request.
	Request(AnyRequest),
	/// A response to a request.
	Response(AnyResponse),
	/// A notification (no response expected).
	Notification(AnyNotification),
}

impl Message {
	/// HTTP header name for content length.
	const CONTENT_LENGTH: &'static str = "Content-Length";

	/// Reads a complete JSON-RPC message from the input stream.
	pub async fn read(mut reader: impl futures::AsyncBufRead + Unpin) -> Result<Self> {
		let mut line = String::new();
		let mut content_len = None;
		loop {
			line.clear();
			reader.read_line(&mut line).await?;
			if line.is_empty() {
				return Err(Error::Eof);
			}
			if line == "\r\n" {
				break;
			}
			// NB. LSP spec is stricter than HTTP spec, the spaces here is required and it's not
			// explicitly permitted to include extra spaces. We reject them here.
			let (name, value) = line
				.strip_suffix("\r\n")
				.and_then(|line| line.split_once(": "))
				.ok_or_else(|| Error::Protocol(format!("Invalid header: {line:?}")))?;
			if name.eq_ignore_ascii_case(Self::CONTENT_LENGTH) {
				let value = value
					.parse::<usize>()
					.map_err(|_| Error::Protocol(format!("Invalid content-length: {value}")))?;
				content_len = Some(value);
			}
		}
		let content_len =
			content_len.ok_or_else(|| Error::Protocol("Missing content-length".into()))?;
		let mut buf = vec![0u8; content_len];
		reader.read_exact(&mut buf).await?;
		::tracing::trace!(msg = %String::from_utf8_lossy(&buf), "incoming");
		let msg = serde_json::from_slice::<RawMessage<Self>>(&buf)?;
		Ok(msg.inner)
	}

	/// Writes this message to the output stream with HTTP headers.
	pub async fn write(&self, mut writer: impl futures::AsyncWrite + Unpin) -> Result<()> {
		let buf = serde_json::to_string(&RawMessage::new(self))?;
		::tracing::trace!(msg = %buf, "outgoing");
		writer
			.write_all(format!("{}: {}\r\n\r\n", Self::CONTENT_LENGTH, buf.len()).as_bytes())
			.await?;
		writer.write_all(buf.as_bytes()).await?;
		writer.flush().await?;
		Ok(())
	}
}
