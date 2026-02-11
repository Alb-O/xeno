//! Core LSP types: error codes, requests, responses, notifications.

use std::fmt;

pub use lsp_types::NumberOrString;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;

/// A JSON-RPC error code.
///
/// Codes defined and/or used by LSP are defined as associated constants, eg.
/// [`ErrorCode::REQUEST_FAILED`].
///
/// See:
/// <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#errorCodes>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Error)]
#[error("jsonrpc error {0}")]
pub struct ErrorCode(pub i32);

impl From<i32> for ErrorCode {
	fn from(i: i32) -> Self {
		Self(i)
	}
}

impl ErrorCode {
	/// Invalid JSON was received by the server. An error occurred on the server while parsing the
	/// JSON text.
	///
	/// Defined by [JSON-RPC](https://www.jsonrpc.org/specification#error_object).
	pub const PARSE_ERROR: Self = Self(-32700);

	/// The JSON sent is not a valid Request object.
	///
	/// Defined by [JSON-RPC](https://www.jsonrpc.org/specification#error_object).
	pub const INVALID_REQUEST: Self = Self(-32600);

	/// The method does not exist / is not available.
	///
	/// Defined by [JSON-RPC](https://www.jsonrpc.org/specification#error_object).
	pub const METHOD_NOT_FOUND: Self = Self(-32601);

	/// Invalid method parameter(s).
	///
	/// Defined by [JSON-RPC](https://www.jsonrpc.org/specification#error_object).
	pub const INVALID_PARAMS: Self = Self(-32602);

	/// Internal JSON-RPC error.
	///
	/// Defined by [JSON-RPC](https://www.jsonrpc.org/specification#error_object).
	pub const INTERNAL_ERROR: Self = Self(-32603);

	/// This is the start range of JSON-RPC reserved error codes.
	/// It doesn't denote a real error code. No LSP error codes should
	/// be defined between the start and end range. For backwards
	/// compatibility the `ServerNotInitialized` and the `UnknownErrorCode`
	/// are left in the range.
	///
	/// @since 3.16.0
	pub const JSONRPC_RESERVED_ERROR_RANGE_START: Self = Self(-32099);

	/// Error code indicating that a server received a notification or
	/// request before the server has received the `initialize` request.
	pub const SERVER_NOT_INITIALIZED: Self = Self(-32002);

	/// (Defined by LSP specification without description)
	pub const UNKNOWN_ERROR_CODE: Self = Self(-32001);

	/// This is the end range of JSON-RPC reserved error codes.
	/// It doesn't denote a real error code.
	///
	/// @since 3.16.0
	pub const JSONRPC_RESERVED_ERROR_RANGE_END: Self = Self(-32000);

	/// This is the start range of LSP reserved error codes.
	/// It doesn't denote a real error code.
	///
	/// @since 3.16.0
	pub const LSP_RESERVED_ERROR_RANGE_START: Self = Self(-32899);

	/// A request failed but it was syntactically correct, e.g the
	/// method name was known and the parameters were valid. The error
	/// message should contain human readable information about why
	/// the request failed.
	///
	/// @since 3.17.0
	pub const REQUEST_FAILED: Self = Self(-32803);

	/// The server cancelled the request. This error code should
	/// only be used for requests that explicitly support being
	/// server cancellable.
	///
	/// @since 3.17.0
	pub const SERVER_CANCELLED: Self = Self(-32802);

	/// The server detected that the content of a document got
	/// modified outside normal conditions. A server should
	/// NOT send this error code if it detects a content change
	/// in it unprocessed messages. The result even computed
	/// on an older state might still be useful for the client.
	///
	/// If a client decides that a result is not of any use anymore
	/// the client should cancel the request.
	pub const CONTENT_MODIFIED: Self = Self(-32801);

	/// The client has canceled a request and a server as detected
	/// the cancel.
	pub const REQUEST_CANCELLED: Self = Self(-32800);

	/// This is the end range of LSP reserved error codes.
	/// It doesn't denote a real error code.
	///
	/// @since 3.16.0
	pub const LSP_RESERVED_ERROR_RANGE_END: Self = Self(-32800);
}

/// The identifier of requests and responses.
///
/// Though `null` is technically a valid id for responses, we reject it since it hardly makes sense
/// for valid communication.
pub type RequestId = NumberOrString;

/// The error object in case a request fails.
///
/// See:
/// <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#responseError>
#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[non_exhaustive]
#[error("{message} ({code})")]
pub struct ResponseError {
	/// A number indicating the error type that occurred.
	pub code: ErrorCode,
	/// A string providing a short description of the error.
	pub message: String,
	/// Structured value that contains additional information about the error.
	pub data: Option<JsonValue>,
}

impl ResponseError {
	/// Create a new error object with a JSON-RPC error code and a message.
	#[must_use]
	pub fn new(code: ErrorCode, message: impl fmt::Display) -> Self {
		Self {
			code,
			message: message.to_string(),
			data: None,
		}
	}

	/// Create a new error object with a JSON-RPC error code, a message, and additional data.
	#[must_use]
	pub fn new_with_data(code: ErrorCode, message: impl fmt::Display, data: JsonValue) -> Self {
		Self {
			code,
			message: message.to_string(),
			data: Some(data),
		}
	}
}

/// A dynamic runtime [LSP request](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#requestMessage).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AnyRequest {
	/// The request id.
	pub id: RequestId,
	/// The method to be invoked.
	pub method: String,
	/// The method's params.
	#[serde(default)]
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	pub params: serde_json::Value,
}

/// A dynamic runtime [LSP notification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#notificationMessage).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AnyNotification {
	/// The method to be invoked.
	pub method: String,
	/// The notification's params.
	#[serde(default)]
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	pub params: JsonValue,
}

impl AnyNotification {
	/// Create a new notification with the given method and params.
	#[must_use]
	pub fn new(method: impl Into<String>, params: JsonValue) -> Self {
		Self { method: method.into(), params }
	}
}

/// A dynamic runtime [LSP response](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#responseMessage).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AnyResponse {
	/// Request ID this response corresponds to.
	pub id: RequestId,
	/// Result value on success (mutually exclusive with `error`).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub result: Option<JsonValue>,
	/// Error object on failure (mutually exclusive with `result`).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<ResponseError>,
}

impl AnyResponse {
	/// Create a new successful response.
	#[must_use]
	pub fn new_ok(id: RequestId, result: JsonValue) -> Self {
		Self {
			id,
			result: Some(result),
			error: None,
		}
	}

	/// Create a new error response.
	#[must_use]
	pub fn new_err(id: RequestId, error: ResponseError) -> Self {
		Self {
			id,
			result: None,
			error: Some(error),
		}
	}
}
