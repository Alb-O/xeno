use crate::RegistryEntry;

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

/// Fatal insertion errors (always panic, regardless of policy).
#[derive(Debug, Clone)]
pub enum InsertFatal {
	/// Two definitions have the same `meta.id`.
	DuplicateId {
		key: &'static str,
		existing_id: &'static str,
		new_id: &'static str,
	},
	/// A name or alias shadows an existing ID.
	KeyShadowsId {
		kind: KeyKind,
		key: &'static str,
		id_owner: &'static str,
		new_id: &'static str,
	},
}

impl std::fmt::Display for InsertFatal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			InsertFatal::DuplicateId {
				key,
				existing_id,
				new_id,
			} => {
				write!(
					f,
					"duplicate ID: key={key:?} existing={existing_id} new={new_id}"
				)
			}
			InsertFatal::KeyShadowsId {
				kind,
				key,
				id_owner,
				new_id,
			} => {
				write!(
					f,
					"{kind} shadows ID: key={key:?} id_owner={id_owner} from={new_id}"
				)
			}
		}
	}
}

impl std::error::Error for InsertFatal {}

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

/// Inserts a key with proper invariant checking.
pub fn insert_typed_key<T: RegistryEntry + 'static>(
	store: &mut dyn KeyStore<T>,
	registry_label: &'static str,
	choose_winner: ChooseWinner<T>,
	kind: KeyKind,
	key: &'static str,
	def: &'static T,
) -> Result<InsertAction, InsertFatal> {
	match kind {
		KeyKind::Id => {
			if let Some(prev) = store.insert_id(key, def)
				&& !std::ptr::eq(prev, def)
			{
				return Err(InsertFatal::DuplicateId {
					key,
					existing_id: prev.id(),
					new_id: def.id(),
				});
			}
			Ok(InsertAction::InsertedNew)
		}
		KeyKind::Name | KeyKind::Alias => {
			if let Some(id_owner) = store.get_id_owner(key) {
				if !std::ptr::eq(id_owner, def) {
					return Err(InsertFatal::KeyShadowsId {
						kind,
						key,
						id_owner: id_owner.id(),
						new_id: def.id(),
					});
				}
				return Ok(InsertAction::KeptExisting);
			}

			if let Some(existing) = store.get_key_winner(key) {
				if std::ptr::eq(existing, def) {
					return Ok(InsertAction::KeptExisting);
				}

				let new_wins = choose_winner(kind, key, existing, def);
				let (action, winner_id) = if new_wins {
					store.set_key_winner(key, def);
					(InsertAction::ReplacedExisting, def.id())
				} else {
					(InsertAction::KeptExisting, existing.id())
				};

				store.push_collision(Collision {
					kind,
					key,
					existing_id: existing.id(),
					new_id: def.id(),
					winner_id,
					action,
					registry: registry_label,
				});

				Ok(action)
			} else {
				store.set_key_winner(key, def);
				Ok(InsertAction::InsertedNew)
			}
		}
	}
}
