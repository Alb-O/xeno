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
/// # Concurrency
///
/// Must be called synchronously to maintain FIFO ordering with
/// [`reply()`](crate::client::transport::LspTransport::reply).
///
/// # Supported Methods
///
/// * `workspace/configuration`: Server settings from registry metadata
/// * `workspace/workspaceFolders`: Workspace root folder
/// * `client/registerCapability`, `client/unregisterCapability`: No-op success
/// * `window/showMessageRequest`, `window/workDoneProgress/create`: No-op success
/// * `workspace/diagnostic/refresh`: No-op success
/// * `workspace/applyEdit`: Unsupported
///
/// # Errors
///
/// Returns [`ErrorCode::METHOD_NOT_FOUND`] for unsupported methods.
pub async fn handle_server_request(sync: &DocumentSync, server: LanguageServerId, req: AnyRequest) -> Result<JsonValue, ResponseError> {
	tracing::trace!(
		server_id = %server,
		method = %req.method,
		params = ?req.params,
		"Handling server request"
	);

	let result = match req.method.as_str() {
		"workspace/configuration" => handle_workspace_configuration(sync, server, req.params).await,
		"workspace/workspaceFolders" => handle_workspace_folders(sync, server).await,
		"client/registerCapability" => Ok(JsonValue::Null),
		"client/unregisterCapability" => Ok(JsonValue::Null),
		"window/showMessageRequest" => Ok(JsonValue::Null),
		"window/workDoneProgress/create" => Ok(JsonValue::Null),
		"workspace/diagnostic/refresh" => Ok(JsonValue::Null),
		"workspace/applyEdit" => Ok(json!({
			"applied": false,
			"failureReason": "workspace/applyEdit not yet supported"
		})),
		_ => Err(ResponseError::new(
			ErrorCode::METHOD_NOT_FOUND,
			format!("Server request method '{}' not supported", req.method),
		)),
	};

	tracing::trace!(
		server_id = %server,
		method = %req.method,
		result = ?result,
		"Server request result"
	);

	result
}

/// Handle `workspace/configuration` request.
///
/// Returns configuration array aligned with requested items. Supports section-based
/// slicing where items specify a `section` field (e.g., `"rust-analyzer"`).
/// Empty object returned when section or settings not found for compatibility.
async fn handle_workspace_configuration(sync: &DocumentSync, server: LanguageServerId, params: JsonValue) -> Result<JsonValue, ResponseError> {
	let settings = sync.registry().get_server_meta(server).and_then(|m| m.settings).unwrap_or_else(|| json!({}));

	let items = params.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();

	let result: Vec<JsonValue> = items
		.iter()
		.map(|item| -> JsonValue {
			if let Some(section) = item.get("section").and_then(|s| s.as_str()) {
				if let Some(section_value) = settings.get(section) {
					section_value.clone()
				} else {
					json!({})
				}
			} else {
				// No section specified, return full settings
				settings.clone()
			}
		})
		.collect();

	Ok(JsonValue::Array(result))
}

/// Handle `workspace/workspaceFolders` request.
///
/// Returns workspace folder array with percent-encoded `file://` URI.
/// Empty array returned if server metadata unavailable or URI conversion fails.
async fn handle_workspace_folders(sync: &DocumentSync, server: LanguageServerId) -> Result<JsonValue, ResponseError> {
	let Some(meta) = sync.registry().get_server_meta(server) else {
		return Ok(json!([]));
	};

	let uri_str = match url::Url::from_file_path(&meta.root_path) {
		Ok(url) => url.to_string(),
		Err(_) => return Ok(json!([])),
	};

	let name = meta.root_path.file_name().and_then(|n: &std::ffi::OsStr| n.to_str()).unwrap_or("workspace");

	Ok(json!([{
		"uri": uri_str,
		"name": name
	}]))
}
