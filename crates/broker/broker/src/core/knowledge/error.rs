use std::fmt;

use helix_db::helix_engine::types::EngineError;

#[derive(Debug)]
pub enum KnowledgeError {
	Io(std::io::Error),
	Engine(EngineError),
	MissingStateDir,
}

impl fmt::Display for KnowledgeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
