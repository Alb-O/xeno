//! Server-initiated request handlers.
//!
//! Handles LSP requests from servers to the client, such as configuration queries
//! and capability registration.

use serde_json::{Value as JsonValue, json};

use crate::DocumentSync;
use crate::client::LanguageServerId;
use crate::types::{AnyRequest, ErrorCode, ResponseError};

/// Dispatch a server-initiated LSP request to the appropriate handler.
///
/// Must be called synchronously (not spawned) to maintain FIFO ordering with [`reply()`](crate::client::transport::LspTransport::reply).
/// The broker relies on strict request/reply pairing for correctness.
///
/// # Supported Methods
///
/// - `workspace/configuration`: Returns server settings from registry metadata
/// - `workspace/workspaceFolders`: Returns workspace root folder
/// - `client/registerCapability`, `client/unregisterCapability`: No-op
/// - `window/showMessageRequest`: No-op
/// - `workspace/applyEdit`: Returns failure (not implemented)
///
/// # Errors
///
/// Returns [`ErrorCode::METHOD_NOT_FOUND`] for unsupported methods.
pub async fn handle_server_request(
	sync: &DocumentSync,
	server: LanguageServerId,
	req: AnyRequest,
) -> Result<JsonValue, ResponseError> {
	match req.method.as_str() {
		"workspace/configuration" => handle_workspace_configuration(sync, server, req.params).await,
		"workspace/workspaceFolders" => handle_workspace_folders(sync, server).await,
		"client/registerCapability" => Ok(JsonValue::Null),
		"client/unregisterCapability" => Ok(JsonValue::Null),
		"window/showMessageRequest" => Ok(JsonValue::Null),
		"workspace/applyEdit" => Ok(json!({
			"applied": false,
			"failureReason": "workspace/applyEdit not yet supported"
		})),
		_ => Err(ResponseError::new(
			ErrorCode::METHOD_NOT_FOUND,
			format!("Server request method '{}' not supported", req.method),
		)),
	}
}

/// Handle `workspace/configuration` request from a language server.
///
/// Returns an array of configuration objects, one per requested item.
/// Each element is either the server's configured settings or `null` if unavailable.
async fn handle_workspace_configuration(
	sync: &DocumentSync,
	server: LanguageServerId,
	params: JsonValue,
) -> Result<JsonValue, ResponseError> {
	let meta = sync.registry().get_server_meta(server);
	let items_len = params
		.get("items")
		.and_then(|v| v.as_array())
		.map(|arr| arr.len())
		.unwrap_or(1);

	let result: Vec<JsonValue> = (0..items_len)
		.map(|_| {
			meta.as_ref()
				.and_then(|m| m.settings.clone())
				.unwrap_or(JsonValue::Null)
		})
		.collect();

	Ok(JsonValue::Array(result))
}

/// Handle `workspace/workspaceFolders` request from a language server.
///
/// Returns an array containing a single workspace folder derived from the server's
/// root path, or an empty array if metadata is unavailable.
async fn handle_workspace_folders(
	sync: &DocumentSync,
	server: LanguageServerId,
) -> Result<JsonValue, ResponseError> {
	let Some(meta) = sync.registry().get_server_meta(server) else {
		return Ok(json!([]));
	};

	let name = meta
		.root_path
		.file_name()
		.and_then(|n: &std::ffi::OsStr| n.to_str())
		.unwrap_or("workspace");

	Ok(json!([{
		"uri": format!("file://{}", meta.root_path.display()),
		"name": name
	}]))
}
