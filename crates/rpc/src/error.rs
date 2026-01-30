//! Error types for xeno-rpc.

/// Errors that can occur in the RPC system.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
	/// The service main loop has stopped.
	#[error("service stopped")]
	Stopped,
}

/// A result type with `Error` as the error type.
pub type Result<T> = std::result::Result<T, Error>;
