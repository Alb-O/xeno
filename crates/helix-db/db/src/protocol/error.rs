#[cfg(feature = "server")]
use axum::{body::Body, response::IntoResponse};
#[cfg(feature = "server")]
use reqwest::header::CONTENT_TYPE;
use serde::Serialize;
use thiserror::Error;

use crate::helix_engine::types::EngineError;
#[cfg(feature = "server")]
use crate::helix_engine::types::{StorageError, TraversalError, VectorError};
use crate::protocol::request::RequestType;

#[cfg(feature = "server")]
#[derive(Serialize)]
struct ErrorResponse {
	error: String,
	code: &'static str,
}

#[derive(Debug, Error)]
pub enum HelixError {
	#[error("{0}")]
	Engine(#[from] EngineError),
	#[error("Couldn't find `{name}` of type {ty:?}")]
	NotFound { ty: RequestType, name: String },
	#[error("Invalid API key")]
	InvalidApiKey,
}

impl Serialize for HelixError {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.collect_str(&self.to_string())
	}
}

#[cfg(feature = "server")]
impl HelixError {
	fn code(&self) -> &'static str {
		match self {
			HelixError::Engine(EngineError::Vector(_)) => "VECTOR_ERROR",
			HelixError::Engine(_) => "GRAPH_ERROR",
			HelixError::NotFound { .. } => "NOT_FOUND",
			HelixError::InvalidApiKey => "INVALID_API_KEY",
		}
	}
}

#[cfg(feature = "server")]
impl IntoResponse for HelixError {
	fn into_response(self) -> axum::response::Response {
		let status = match &self {
			HelixError::NotFound { .. }
			| HelixError::Engine(EngineError::Storage(StorageError::ConfigFileNotFound))
			| HelixError::Engine(EngineError::Traversal(
				TraversalError::NodeNotFound
				| TraversalError::EdgeNotFound
				| TraversalError::LabelNotFound
				| TraversalError::ShortestPathNotFound,
			))
			| HelixError::Engine(EngineError::Vector(VectorError::VectorNotFound(_))) => {
				axum::http::StatusCode::NOT_FOUND
			}
			HelixError::Engine(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
			HelixError::InvalidApiKey => axum::http::StatusCode::FORBIDDEN,
			#[cfg(not(feature = "server"))]
			_ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
		};

		let error_response = ErrorResponse {
			error: self.to_string(),
			code: self.code(),
		};

		let body = sonic_rs::to_vec(&error_response).unwrap_or_else(|_| {
			br#"{"error":"Internal serialization error","code":"INTERNAL_ERROR"}"#.to_vec()
		});

		axum::response::Response::builder()
			.status(status)
			.header(CONTENT_TYPE, "application/json")
			.body(Body::from(body))
			.unwrap_or_else(|e| {
				// This should never happen with valid HTTP headers, but handle gracefully
				tracing::error!("Failed to build error response: {e:?}");
				axum::response::Response::builder()
					.status(500)
					.body(Body::from(
						r#"{"error":"Internal server error","code":"INTERNAL_ERROR"}"#,
					))
					.expect("static response should always build")
			})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::helix_engine::types::{StorageError, VectorError};

	// ============================================================================
	// HelixError Variant Tests
	// ============================================================================

	#[test]
	fn test_helix_error_not_found() {
		let error = HelixError::NotFound {
			ty: RequestType::Query,
			name: "test_query".to_string(),
		};

		let error_string = error.to_string();
		assert!(error_string.contains("test_query"));
		assert!(error_string.contains("Couldn't find"));
	}

	#[test]
	fn test_helix_error_not_found_mcp() {
		let error = HelixError::NotFound {
			ty: RequestType::MCP,
			name: "test_mcp".to_string(),
		};

		let error_string = error.to_string();
		assert!(error_string.contains("test_mcp"));
		assert!(error_string.contains("MCP"));
	}

	#[test]
	fn test_helix_error_graph() {
		let graph_err = EngineError::Storage(StorageError::Decode("test decode error".to_string()));
		let helix_err = HelixError::from(graph_err);

		assert!(matches!(helix_err, HelixError::Engine(_)));
		let error_string = helix_err.to_string();
		assert!(error_string.contains("test decode error"));
	}

	#[test]
	fn test_helix_error_vector() {
		let vector_err = VectorError::InvalidVectorLength;
		let helix_err = HelixError::from(EngineError::from(vector_err));

		assert!(matches!(helix_err, HelixError::Engine(_)));
	}

	// ============================================================================
	// IntoResponse + Code Tests (server feature)
	// ============================================================================

	#[cfg(feature = "server")]
	mod server {
		use reqwest::header::CONTENT_TYPE;

		use super::*;
		use crate::helix_engine::types::TraversalError;

		#[test]
		fn test_helix_error_into_response_not_found() {
			let error = HelixError::NotFound {
				ty: RequestType::Query,
				name: "missing".to_string(),
			};

			let response = error.into_response();
			assert_eq!(response.status(), 404);
			assert_eq!(
				response.headers().get(CONTENT_TYPE).unwrap(),
				"application/json"
			);
		}

		#[test]
		fn test_helix_error_into_response_graph_error() {
			let graph_err = EngineError::Storage(StorageError::Decode("decode failed".to_string()));
			let helix_err = HelixError::from(graph_err);

			let response = helix_err.into_response();
			assert_eq!(response.status(), 500);
			assert_eq!(
				response.headers().get(CONTENT_TYPE).unwrap(),
				"application/json"
			);
		}

		#[test]
		fn test_helix_error_into_response_vector_error() {
			let vector_err = VectorError::InvalidVectorData;
			let helix_err = HelixError::from(EngineError::from(vector_err));

			let response = helix_err.into_response();
			assert_eq!(response.status(), 500);
			assert_eq!(
				response.headers().get(CONTENT_TYPE).unwrap(),
				"application/json"
			);
		}

		#[test]
		fn test_helix_error_code_graph() {
			let error = HelixError::Engine(TraversalError::NodeNotFound.into());
			assert_eq!(error.code(), "GRAPH_ERROR");
		}

		#[test]
		fn test_helix_error_code_vector() {
			let error = HelixError::Engine(EngineError::from(VectorError::InvalidVectorLength));
			assert_eq!(error.code(), "VECTOR_ERROR");
		}

		#[test]
		fn test_helix_error_code_not_found() {
			let error = HelixError::NotFound {
				ty: RequestType::Query,
				name: "test".to_string(),
			};
			assert_eq!(error.code(), "NOT_FOUND");
		}

		#[test]
		fn test_helix_error_code_invalid_api_key() {
			let error = HelixError::InvalidApiKey;
			assert_eq!(error.code(), "INVALID_API_KEY");
		}
	}

	// ============================================================================
	// Error Trait Tests
	// ============================================================================

	#[test]
	fn test_helix_error_is_error_trait() {
		let error = HelixError::NotFound {
			ty: RequestType::Query,
			name: "test".to_string(),
		};

		// Test that it implements std::error::Error
		fn assert_error<T: std::error::Error>(_: T) {}
		assert_error(error);
	}

	#[test]
	fn test_helix_error_debug() {
		let error = HelixError::NotFound {
			ty: RequestType::Query,
			name: "debug_test".to_string(),
		};

		let debug_str = format!("{:?}", error);
		assert!(debug_str.contains("NotFound"));
		assert!(debug_str.contains("debug_test"));
	}

	// ============================================================================
	// InvalidApiKey Tests
	// ============================================================================

	#[test]
	fn test_helix_error_invalid_api_key() {
		let error = HelixError::InvalidApiKey;
		let error_string = error.to_string();
		assert_eq!(error_string, "Invalid API key");
	}

	#[test]
	#[cfg(feature = "server")]
	fn test_helix_error_invalid_api_key_into_response() {
		let error = HelixError::InvalidApiKey;
		let response = error.into_response();
		assert_eq!(response.status(), 403);
	}

	#[test]
	fn test_helix_error_invalid_api_key_debug() {
		let error = HelixError::InvalidApiKey;
		let debug_str = format!("{:?}", error);
		assert!(debug_str.contains("InvalidApiKey"));
	}
}
