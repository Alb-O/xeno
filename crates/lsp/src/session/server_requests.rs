//! Server-initiated request handlers.
//!
//! Handles LSP requests from servers to the client, such as configuration queries
//! and capability registration.

use std::time::Duration;

use serde_json::{Value as JsonValue, json};

use crate::DocumentSync;
use crate::client::LanguageServerId;
use crate::types::{AnyRequest, ErrorCode, ResponseError};

/// Timeout for workspace/applyEdit requests.
///
/// Used both for the server-side `tokio::time::timeout` and the editor-side
/// deadline on [`ApplyEditRequest`](crate::sync::ApplyEditRequest). Must be
/// the same value so the deadline check in the editor drain is consistent
/// with the server's timeout behavior.
pub(crate) const APPLY_EDIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Structured reply shape before conversion into wire-level JSON-RPC result.
pub(crate) enum ServerRequestReply {
	Json(JsonValue),
	MethodNotFound,
}

impl ServerRequestReply {
	fn into_result(self, method: &str) -> Result<JsonValue, ResponseError> {
		match self {
			Self::Json(value) => Ok(value),
			Self::MethodNotFound => Err(ResponseError::new(
				ErrorCode::METHOD_NOT_FOUND,
				format!("Server request method '{method}' not supported"),
			)),
		}
	}
}

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
/// * `workspace/applyEdit`: Route to editor for text edit application
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

	let reply = dispatch_server_request(sync, server, req.method.as_str(), req.params).await;
	let result = reply.into_result(req.method.as_str());

	tracing::trace!(
		server_id = %server,
		method = %req.method,
		result = ?result,
		"Server request result"
	);

	result
}

/// Dispatch a server request into a typed reply model.
pub(crate) async fn dispatch_server_request(sync: &DocumentSync, server: LanguageServerId, method: &str, params: JsonValue) -> ServerRequestReply {
	match method {
		"workspace/configuration" => ServerRequestReply::Json(handle_workspace_configuration(sync, server, params).await),
		"workspace/workspaceFolders" => ServerRequestReply::Json(handle_workspace_folders(sync, server).await),
		"client/registerCapability" => ServerRequestReply::Json(JsonValue::Null),
		"client/unregisterCapability" => ServerRequestReply::Json(JsonValue::Null),
		"window/showMessageRequest" => ServerRequestReply::Json(JsonValue::Null),
		"window/workDoneProgress/create" => ServerRequestReply::Json(JsonValue::Null),
		"workspace/diagnostic/refresh" => ServerRequestReply::Json(JsonValue::Null),
		"workspace/applyEdit" => ServerRequestReply::Json(handle_workspace_apply_edit(sync, params).await),
		_ => ServerRequestReply::MethodNotFound,
	}
}

/// Handle `workspace/applyEdit` request.
///
/// Routes the edit to the editor via the apply-edit channel and waits for the result.
/// Falls back to `applied: false` if the channel is not configured or times out.
async fn handle_workspace_apply_edit(sync: &DocumentSync, params: JsonValue) -> JsonValue {
	use lsp_types::ApplyWorkspaceEditParams;

	use crate::sync::ApplyEditRequest;

	let Some(tx) = sync.apply_edit_sender() else {
		return json!({ "applied": false, "failureReason": "no editor connected" });
	};

	let Ok(params) = serde_json::from_value::<ApplyWorkspaceEditParams>(params) else {
		return json!({ "applied": false, "failureReason": "invalid params" });
	};

	let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
	let request = ApplyEditRequest {
		edit: params.edit,
		reply: reply_tx,
		deadline: std::time::Instant::now() + APPLY_EDIT_TIMEOUT,
	};

	if tx.send(request).is_err() {
		return json!({ "applied": false, "failureReason": "editor disconnected" });
	}

	// Wait with a timeout to avoid hanging the server request loop.
	match tokio::time::timeout(APPLY_EDIT_TIMEOUT, reply_rx).await {
		Ok(Ok(result)) => {
			let mut response = json!({ "applied": result.applied });
			if let Some(reason) = result.failure_reason {
				response["failureReason"] = json!(reason);
			}
			if let Some(idx) = result.failed_change {
				response["failedChange"] = json!(idx);
			}
			response
		}
		Ok(Err(_)) => json!({ "applied": false, "failureReason": "editor dropped reply" }),
		Err(_) => json!({ "applied": false, "failureReason": "timeout" }),
	}
}

/// Handle `workspace/configuration` request.
///
/// Returns configuration array aligned with requested items. Supports section-based
/// slicing where items specify a `section` field (e.g., `"rust-analyzer"`).
/// Empty object returned when section or settings not found for compatibility.
async fn handle_workspace_configuration(sync: &DocumentSync, server: LanguageServerId, params: JsonValue) -> JsonValue {
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

	JsonValue::Array(result)
}

/// Handle `workspace/workspaceFolders` request.
///
/// Returns workspace folder array with percent-encoded `file://` URI.
/// Empty array returned if server metadata unavailable or URI conversion fails.
async fn handle_workspace_folders(sync: &DocumentSync, server: LanguageServerId) -> JsonValue {
	let Some(meta) = sync.registry().get_server_meta(server) else {
		return json!([]);
	};

	let uri_str = match url::Url::from_file_path(&meta.root_path) {
		Ok(url) => url.to_string(),
		Err(_) => return json!([]),
	};

	let name = meta.root_path.file_name().and_then(|n: &std::ffi::OsStr| n.to_str()).unwrap_or("workspace");

	json!([{
		"uri": uri_str,
		"name": name
	}])
}
