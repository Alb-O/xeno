use std::collections::HashMap;

use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use super::Outbound;
use crate::client::config::LanguageServerId;
use crate::client::transport::{TransportEvent, TransportStatus};
use crate::protocol::JsonRpcProtocol;
use crate::types::{AnyNotification, AnyRequest, AnyResponse, RequestId, ResponseError};
use crate::{Error, Result};

/// Runs the I/O loop for a single server process.
pub(super) async fn run_server_io(
	id: LanguageServerId,
	mut stdin: tokio::process::ChildStdin,
	stdout: tokio::process::ChildStdout,
	mut outbound_rx: mpsc::UnboundedReceiver<Outbound>,
	event_tx: mpsc::UnboundedSender<TransportEvent>,
) {
	let mut reader = BufReader::new(stdout);
	let mut pending: HashMap<RequestId, oneshot::Sender<Result<AnyResponse>>> = HashMap::new();
	let protocol = JsonRpcProtocol::new();
	let mut read_buf = String::new();

	loop {
		tokio::select! {
			// Handle all outbound messages sequentially for total ordering
			Some(out) = outbound_rx.recv() => {
				let write_res: Result<()> = match out {
					Outbound::Notify { notif, written } => {
						let r = write_notification(&mut stdin, &protocol, &notif).await;
						if let Some(tx) = written {
							let _ = tx.send(r.clone());
						}
						r
					}
					Outbound::Request { pending: pending_req } => {
						let req_id = pending_req.request.id.clone();
						let r = write_message(&mut stdin, &protocol, &pending_req.request).await;
						match r {
							Ok(()) => {
								pending.insert(req_id, pending_req.response_tx);
								Ok(())
							}
							Err(e) => {
								let _ = pending_req.response_tx.send(Err(e.clone()));
								Err(e)
							}
						}
					}
					Outbound::Reply { reply, written } => {
						let r = write_response(&mut stdin, reply.id, reply.resp).await;
						if let Some(tx) = written {
							let _ = tx.send(r.clone());
						}
						r
					}
				};

				if let Err(e) = write_res {
					// Treat write failure as fatal: terminate IO loop and notify manager
					tracing::error!(server_id = %id, error = %e, "Outbound write failed; terminating IO loop");
					let _ = event_tx.send(TransportEvent::Status {
						server: id,
						status: TransportStatus::Crashed,
					});
					break;
				}
			}

			// Handle inbound messages from server
			result = read_message(&mut reader, &protocol, &mut read_buf) => {
				match result {
					Ok(Some(msg)) => {
						handle_inbound_message(id, msg, &mut pending, &event_tx);
					}
					Ok(None) => {
						// EOF - server stopped
						tracing::info!(server_id = %id, "LSP server closed connection");
						let _ = event_tx.send(TransportEvent::Status {
							server: id,
							status: TransportStatus::Stopped,
						});
						break;
					}
					Err(e) => {
						tracing::error!(server_id = %id, error = %e, "Error reading from LSP server");
						let _ = event_tx.send(TransportEvent::Status {
							server: id,
							status: TransportStatus::Crashed,
						});
						break;
					}
				}
			}
		}
	}

	// Clean up pending requests
	for (_, tx) in pending {
		let _ = tx.send(Err(Error::ServiceStopped));
	}

	// Clean up pending barriers in the outbound queue
	while let Ok(out) = outbound_rx.try_recv() {
		match out {
			Outbound::Notify { written: Some(tx), .. } | Outbound::Reply { written: Some(tx), .. } => {
				let _ = tx.send(Err(Error::ServiceStopped));
			}
			Outbound::Request { pending: p } => {
				let _ = p.response_tx.send(Err(Error::ServiceStopped));
			}
			_ => {}
		}
	}
}

/// Writes a JSON-RPC request to the server's stdin.
async fn write_message(stdin: &mut tokio::process::ChildStdin, _protocol: &JsonRpcProtocol, req: &AnyRequest) -> Result<()> {
	let json = serde_json::to_string(&serde_json::json!({
		"jsonrpc": "2.0",
		"id": req.id,
		"method": req.method,
		"params": req.params,
	}))?;

	let msg = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
	stdin.write_all(msg.as_bytes()).await?;
	stdin.flush().await?;
	Ok(())
}

/// Writes a JSON-RPC notification to the server's stdin.
async fn write_notification(stdin: &mut tokio::process::ChildStdin, _protocol: &JsonRpcProtocol, notif: &AnyNotification) -> Result<()> {
	let json = serde_json::to_string(&serde_json::json!({
		"jsonrpc": "2.0",
		"method": notif.method,
		"params": notif.params,
	}))?;

	let msg = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
	stdin.write_all(msg.as_bytes()).await?;
	stdin.flush().await?;
	Ok(())
}

/// Writes a JSON-RPC response to the server's stdin.
async fn write_response(stdin: &mut tokio::process::ChildStdin, id: RequestId, resp: std::result::Result<JsonValue, ResponseError>) -> Result<()> {
	let obj = match resp {
		Ok(result) => serde_json::json!({
			"jsonrpc": "2.0",
			"id": id,
			"result": result,
		}),
		Err(err) => serde_json::json!({
			"jsonrpc": "2.0",
			"id": id,
			"error": err,
		}),
	};

	let json = serde_json::to_string(&obj)?;
	let msg = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
	stdin.write_all(msg.as_bytes()).await?;
	stdin.flush().await?;
	Ok(())
}

/// Reads a JSON-RPC message from the server's stdout.
async fn read_message(reader: &mut BufReader<tokio::process::ChildStdout>, _protocol: &JsonRpcProtocol, buf: &mut String) -> Result<Option<JsonValue>> {
	// Read headers
	let mut content_length: Option<usize> = None;
	loop {
		buf.clear();
		let bytes_read = reader.read_line(buf).await?;
		if bytes_read == 0 {
			return Ok(None); // EOF
		}

		let line = buf.trim();
		if line.is_empty() {
			break;
		}

		if let Some(len_str) = line.strip_prefix("Content-Length: ") {
			content_length = len_str.parse().ok();
		}
	}

	let length = content_length.ok_or_else(|| Error::Protocol("missing Content-Length".into()))?;

	// Read body
	let mut body = vec![0u8; length];
	tokio::io::AsyncReadExt::read_exact(reader, &mut body).await?;

	let json: JsonValue = serde_json::from_slice(&body)?;
	Ok(Some(json))
}

/// Handles an inbound message from the server.
fn handle_inbound_message(
	id: LanguageServerId,
	msg: JsonValue,
	pending: &mut HashMap<RequestId, oneshot::Sender<Result<AnyResponse>>>,
	event_tx: &mpsc::UnboundedSender<TransportEvent>,
) {
	// Check if it's a response (has "id" but no "method")
	if msg.get("id").is_some() && msg.get("method").is_none() {
		let resp: AnyResponse = match serde_json::from_value(msg) {
			Ok(r) => r,
			Err(e) => {
				tracing::warn!(server_id = %id, error = %e, "Failed to parse response");
				return;
			}
		};

		if let Some(tx) = pending.remove(&resp.id) {
			let _ = tx.send(Ok(resp));
		}
		return;
	}

	// Check if it's a notification (has "method" but no "id")
	if msg.get("method").is_some() && msg.get("id").is_none() {
		let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
		let params = msg.get("params").cloned().unwrap_or(JsonValue::Null);

		// Handle diagnostics specially
		if method == "textDocument/publishDiagnostics"
			&& let Some(uri) = params.get("uri").and_then(|u| u.as_str())
		{
			let version = params.get("version").and_then(|v| v.as_u64()).map(|v| v as u32);
			let diagnostics = params.get("diagnostics").cloned().unwrap_or(JsonValue::Array(vec![]));
			let _ = event_tx.send(TransportEvent::Diagnostics {
				server: id,
				uri: uri.to_string(),
				version,
				diagnostics,
			});
			return;
		}

		// Other notifications go through as messages
		let _ = event_tx.send(TransportEvent::Message {
			server: id,
			message: crate::Message::Notification(AnyNotification {
				method: method.to_string(),
				params,
			}),
		});
		return;
	}

	// It's a server-initiated request (has both "id" and "method")
	if msg.get("id").is_some() && msg.get("method").is_some() {
		let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
		let params = msg.get("params").cloned().unwrap_or(JsonValue::Null);
		let req_id = msg.get("id").cloned().unwrap_or(JsonValue::Null);

		let id_parsed = match req_id {
			JsonValue::Number(n) => RequestId::Number(n.as_i64().unwrap_or(0) as i32),
			JsonValue::String(s) => RequestId::String(s),
			_ => RequestId::Number(0),
		};

		let _ = event_tx.send(TransportEvent::Message {
			server: id,
			message: crate::Message::Request(AnyRequest {
				id: id_parsed,
				method: method.to_string(),
				params,
			}),
		});
	}
}
