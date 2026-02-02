use serde::Serialize;
use thiserror::Error;

use crate::helix_engine::types::EngineError;
use crate::protocol::request::RequestType;

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
	fn test_helix_error_invalid_api_key_debug() {
		let error = HelixError::InvalidApiKey;
		let debug_str = format!("{:?}", error);
		assert!(debug_str.contains("InvalidApiKey"));
	}
}
