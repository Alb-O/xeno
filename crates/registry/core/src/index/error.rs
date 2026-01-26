use super::collision::KeyKind;
use crate::RegistryEntry;

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

/// Fatal insertion errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum InsertFatal {
	/// Two definitions have the same `meta.id`.
	#[error("duplicate ID: key={key:?} existing={existing_id} new={new_id}")]
	DuplicateId {
		key: &'static str,
		existing_id: &'static str,
		new_id: &'static str,
	},
	/// A name or alias shadows an existing ID.
	#[error("{kind} shadows ID: key={key:?} id_owner={id_owner} from={new_id}")]
	KeyShadowsId {
		kind: KeyKind,
		key: &'static str,
		id_owner: &'static str,
		new_id: &'static str,
	},
}

/// Generic registry error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RegistryError {
	#[error("fatal insertion error: {0}")]
	Insert(#[from] InsertFatal),

	#[error("lock poisoned")]
	Poisoned,
}
