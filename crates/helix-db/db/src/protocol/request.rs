use bytes::Bytes;
use serde::Serialize;
use tokio::sync::oneshot;

use crate::protocol::{Format, HelixError, Response};

pub type RetChan = oneshot::Sender<Result<Response, HelixError>>;

pub type ReqMsg = (Request, RetChan);

#[derive(Debug, Clone)]
pub struct Request {
	pub name: String,
	pub req_type: RequestType,
	pub api_key: Option<String>,
	/// This contains the input parameters serialized with in_fmt
	pub body: Bytes,
	pub in_fmt: Format,
	pub out_fmt: Format,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum RequestType {
	Query,
	MCP,
}

#[cfg(test)]
mod tests {
	use super::*;

	// ============================================================================
	// Request Construction Tests
	// ============================================================================

	#[test]
	fn test_request_construction() {
		let body = Bytes::from("test body");
		let request = Request {
			name: "test_query".to_string(),
			req_type: RequestType::Query,
			api_key: None,
			body: body.clone(),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		assert_eq!(request.name, "test_query");
		assert!(matches!(request.req_type, RequestType::Query));
		assert_eq!(request.body, body);
	}

	#[test]
	fn test_request_clone() {
		let body = Bytes::from("clone test");
		let request = Request {
			name: "original".to_string(),
			req_type: RequestType::MCP,
			api_key: None,
			body: body.clone(),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		let cloned = request.clone();
		assert_eq!(cloned.name, request.name);
		assert_eq!(cloned.body, request.body);
	}

	#[test]
	fn test_request_debug() {
		let request = Request {
			name: "debug_test".to_string(),
			req_type: RequestType::Query,
			api_key: None,
			body: Bytes::from("test"),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		let debug_str = format!("{:?}", request);
		assert!(debug_str.contains("debug_test"));
		assert!(debug_str.contains("Query"));
	}

	// ============================================================================
	// RequestType Tests
	// ============================================================================

	#[test]
	fn test_request_type_query() {
		let rt = RequestType::Query;
		assert!(matches!(rt, RequestType::Query));

		let debug_str = format!("{:?}", rt);
		assert!(debug_str.contains("Query"));
	}

	#[test]
	fn test_request_type_mcp() {
		let rt = RequestType::MCP;
		assert!(matches!(rt, RequestType::MCP));

		let debug_str = format!("{:?}", rt);
		assert!(debug_str.contains("MCP"));
	}

	#[test]
	fn test_request_type_copy() {
		let rt1 = RequestType::Query;
		let rt2 = rt1; // Copy trait

		// Both should be usable
		assert!(matches!(rt1, RequestType::Query));
		assert!(matches!(rt2, RequestType::Query));
	}

	#[test]
	fn test_request_type_clone() {
		let rt1 = RequestType::MCP;
		let rt2 = rt1;

		assert!(matches!(rt1, RequestType::MCP));
		assert!(matches!(rt2, RequestType::MCP));
	}

	// ============================================================================
	// Request with Different Content
	// ============================================================================

	#[test]
	fn test_request_empty_body() {
		let request = Request {
			name: "empty_body".to_string(),
			req_type: RequestType::Query,
			api_key: None,
			body: Bytes::new(),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		assert!(request.body.is_empty());
	}

	#[test]
	fn test_request_large_body() {
		let large_data = vec![0u8; 10_000];
		let body = Bytes::from(large_data.clone());

		let request = Request {
			name: "large_body".to_string(),
			req_type: RequestType::Query,
			api_key: None,
			body: body.clone(),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		assert_eq!(request.body.len(), 10_000);
	}

	#[test]
	fn test_request_utf8_name() {
		let request = Request {
			name: "test_世界_query".to_string(),
			req_type: RequestType::Query,
			api_key: None,
			body: Bytes::from("test"),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		assert!(request.name.contains("世界"));
	}

	// ============================================================================
	// API Key Tests
	// ============================================================================

	#[cfg(feature = "api-key")]
	#[test]
	fn test_request_with_api_key() {
		let key = "my-secret-api-key".to_string();
		let request = Request {
			name: "secure_query".to_string(),
			req_type: RequestType::Query,
			api_key: Some(key.clone()),
			body: Bytes::from("test"),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		assert!(request.api_key.is_some());
		assert_eq!(request.api_key.unwrap(), key);
	}

	#[cfg(feature = "api-key")]
	#[test]
	fn test_api_key_different_values() {
		let key1 = "api-key-1".to_string();
		let key2 = "api-key-2".to_string();

		let request1 = Request {
			name: "test1".to_string(),
			req_type: RequestType::Query,
			api_key: Some(key1.clone()),
			body: Bytes::from("test"),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		let request2 = Request {
			name: "test2".to_string(),
			req_type: RequestType::Query,
			api_key: Some(key2.clone()),
			body: Bytes::from("test"),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		assert_ne!(request1.api_key.unwrap(), request2.api_key.unwrap());
	}

	#[test]
	fn test_request_without_api_key() {
		let request = Request {
			name: "unsecured_query".to_string(),
			req_type: RequestType::Query,
			api_key: None,
			body: Bytes::from("test"),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		assert!(request.api_key.is_none());
	}

	#[cfg(feature = "api-key")]
	#[test]
	fn test_api_key_clone() {
		let key = "test-api-key".to_string();
		let request = Request {
			name: "test".to_string(),
			req_type: RequestType::Query,
			api_key: Some(key),
			body: Bytes::from("test"),
			in_fmt: Format::Json,
			out_fmt: Format::Json,
		};

		let cloned = request.clone();
		assert_eq!(cloned.api_key, request.api_key);
	}
}
