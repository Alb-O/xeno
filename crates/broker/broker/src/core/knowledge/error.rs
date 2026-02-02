//! Error types for the knowledge system.

use helix_db::helix_engine::types::EngineError;

/// Errors returned by the knowledge core and indexer.
#[derive(Debug)]
pub enum KnowledgeError {
	/// Generic I/O failure.
	Io(std::io::Error),
	/// Database engine failure.
	Engine(EngineError),
	/// Could not determine the user state directory for the DB.
	MissingStateDir,
}

impl std::fmt::Display for KnowledgeError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Io(err) => write!(f, "{err}"),
			Self::Engine(err) => write!(f, "{err}"),
			Self::MissingStateDir => write!(f, "unable to resolve state directory"),
		}
	}
}

impl std::error::Error for KnowledgeError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Io(err) => Some(err),
			Self::Engine(err) => Some(err),
			Self::MissingStateDir => None,
		}
	}
}

impl From<std::io::Error> for KnowledgeError {
	fn from(err: std::io::Error) -> Self {
		Self::Io(err)
	}
}

impl From<EngineError> for KnowledgeError {
	fn from(err: EngineError) -> Self {
		Self::Engine(err)
	}
}
