use crate::RegistryEntry;
use crate::error::InsertAction;

/// Distinguishes the type of key being inserted.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum KeyKind {
	/// The definition's unique identifier (`meta.id`).
	Id,
	/// The definition's human-readable name (`meta.name`).
	Name,
	/// An alternative lookup name (`meta.aliases`).
	Alias,
}

impl std::fmt::Display for KeyKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			KeyKind::Id => write!(f, "id"),
			KeyKind::Name => write!(f, "name"),
			KeyKind::Alias => write!(f, "alias"),
		}
	}
}

/// Records a non-fatal collision (name/alias conflicts resolved by policy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
	/// What kind of key collided.
	pub kind: KeyKind,
	/// The colliding key string.
	pub key: &'static str,
	/// The ID of the definition that already held this key.
	pub existing_id: &'static str,
	/// The ID of the new definition trying to claim this key.
	pub new_id: &'static str,
	/// Which definition won.
	pub winner_id: &'static str,
	/// What action was taken.
	pub action: InsertAction,
	/// The registry label where this collision occurred.
	pub registry: &'static str,
}

/// Winner selection function: returns `true` if `new` should replace `existing`.
pub type ChooseWinner<T> =
	fn(kind: KeyKind, key: &'static str, existing: &'static T, new: &'static T) -> bool;

/// Abstraction over key storage for shared insertion logic.
pub trait KeyStore<T: RegistryEntry + 'static> {
	/// Returns the definition that owns this string as an ID, if any.
	fn get_id_owner(&self, id: &str) -> Option<&'static T>;

	/// Returns the current winner in the name/alias namespace.
	fn get_key_winner(&self, key: &str) -> Option<&'static T>;

	/// Sets the winner in the name/alias namespace.
	fn set_key_winner(&mut self, key: &'static str, def: &'static T);

	/// Inserts into the ID table. Returns the previous occupant if any.
	fn insert_id(&mut self, id: &'static str, def: &'static T) -> Option<&'static T>;

	/// Sets the owner of an ID, overwriting any previous owner.
	fn set_id_owner(&mut self, id: &'static str, def: &'static T);

	/// Evicts all keys (name, alias, etc.) that point to the given definition.
	fn evict_def(&mut self, def: &'static T);

	/// Records a collision for diagnostics.
	fn push_collision(&mut self, c: Collision);
}

/// Policy for handling duplicate keys during index construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DuplicatePolicy {
	/// Panic with detailed error message.
	Panic,
	/// Keep the first definition seen for a key.
	FirstWins,
	/// Overwrite with the last definition seen.
	LastWins,
	/// Select winner by priority (higher wins), then source rank, then ID.
	#[default]
	ByPriority,
}

impl DuplicatePolicy {
	/// Returns the appropriate policy based on build configuration.
	#[inline]
	pub fn for_build() -> Self {
		if cfg!(debug_assertions) {
			DuplicatePolicy::Panic
		} else {
			DuplicatePolicy::ByPriority
		}
	}
}
