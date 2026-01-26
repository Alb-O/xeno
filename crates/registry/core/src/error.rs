use thiserror::Error;

use super::capability::Capability;

/// Errors that can occur during command execution.
///
/// This error type is shared between the command and action registries to avoid
/// circular dependencies. Actions re-export this type for convenience.
#[derive(Error, Debug, Clone)]
pub enum CommandError {
	/// General command failure with message.
	#[error("{0}")]
	Failed(String),
	/// A required argument was not provided.
	#[error("missing argument: {0}")]
	MissingArgument(&'static str),
	/// An argument was provided but invalid.
	#[error("invalid argument: {0}")]
	InvalidArgument(String),
	/// File I/O operation failed.
	#[error("I/O error: {0}")]
	Io(String),
	/// Command name was not found in registry.
	#[error("command not found: {0}")]
	NotFound(String),
	/// Command requires a capability the context doesn't provide.
	#[error("missing capability: {0:?}")]
	MissingCapability(Capability),
	/// Operation not supported in current context.
	#[error("unsupported operation: {0}")]
	Unsupported(&'static str),
	/// Catch-all for other errors.
	#[error("{0}")]
	Other(String),
}
