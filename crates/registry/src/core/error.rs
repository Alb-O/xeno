use thiserror::Error;

use super::index::KeyKind;

/// Errors that can occur during command execution.
///
/// This error type is shared between the command and action registries to avoid
/// circular dependencies. Actions re-export this type for convenience.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
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
	/// Operation not supported in current context.
	#[error("unsupported operation: {0}")]
	Unsupported(&'static str),
	/// Catch-all for other errors.
	#[error("{0}")]
	Other(String),
}

/// Fatal insertion errors.
#[derive(Debug, Clone, Error)]
pub enum InsertFatal {
	/// Two definitions have the same `meta.id`.
	#[error("duplicate ID: key={key:?} existing={existing_id} new={new_id}")]
	DuplicateId {
		key: String,
		existing_id: &'static str,
		new_id: &'static str,
	},
	/// A name or alias shadows an existing ID.
	#[error("{kind} shadows ID: key={key:?} id_owner={id_owner} from={new_id}")]
	KeyShadowsId {
		kind: KeyKind,
		key: String,
		id_owner: &'static str,
		new_id: &'static str,
	},
}

/// Generic registry error.
#[derive(Debug, Clone, Error)]
pub enum RegistryError {
	#[error("fatal insertion error: {0}")]
	Insert(#[from] InsertFatal),
	#[error("plugin error: {0}")]
	Plugin(String),
}

/// Result of a successful key insertion.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InsertAction {
	/// Key was new; definition inserted.
	InsertedNew,
	/// Key existed; kept the existing definition (policy chose existing).
	KeptExisting,
	/// Key existed; replaced with new definition (policy chose new).
	ReplacedExisting,
}
