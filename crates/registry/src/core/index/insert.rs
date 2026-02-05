use super::collision::{ChooseWinner, Collision, KeyKind, KeyStore};
use super::types::DefPtr;
use crate::RegistryEntry;
use crate::error::{InsertAction, InsertFatal, RegistryError};

#[inline]
fn r<T: RegistryEntry + 'static>(p: DefPtr<T>) -> &'static T {
	// Safety: pointers stored in Snapshot/Index are valid for the life of the process
	// (builtins) or the life of the snapshot (owned).
	unsafe { p.as_ref() }
}

/// Inserts a key with proper invariant checking.
pub fn insert_typed_key<T: RegistryEntry + 'static>(
	store: &mut dyn KeyStore<T>,
	registry_label: &'static str,
	choose_winner: ChooseWinner<T>,
	kind: KeyKind,
	key: &str,
	def: DefPtr<T>,
) -> Result<InsertAction, InsertFatal> {
	match kind {
		KeyKind::Id => {
			if let Some(prev) = store.insert_id(key, def)
				&& !prev.ptr_eq(def)
			{
				return Err(InsertFatal::DuplicateId {
					key: key.to_string(),
					existing_id: r(prev).id(),
					new_id: r(def).id(),
				});
			}
			Ok(InsertAction::InsertedNew)
		}
		KeyKind::Name | KeyKind::Alias => {
			if let Some(id_owner) = store.get_id_owner(key) {
				if !id_owner.ptr_eq(def) {
					return Err(InsertFatal::KeyShadowsId {
						kind,
						key: key.to_string(),
						id_owner: r(id_owner).id(),
						new_id: r(def).id(),
					});
				}
				return Ok(InsertAction::KeptExisting);
			}

			if let Some(existing) = store.get_key_winner(key) {
				if existing.ptr_eq(def) {
					return Ok(InsertAction::KeptExisting);
				}

				let new_wins = choose_winner(kind, key, r(existing), r(def));
				let (action, winner_id) = if new_wins {
					store.set_key_winner(key, def);
					(InsertAction::ReplacedExisting, r(def).id())
				} else {
					(InsertAction::KeptExisting, r(existing).id())
				};

				store.push_collision(Collision {
					kind,
					key: Box::from(key),
					existing_id: r(existing).id(),
					new_id: r(def).id(),
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

/// Inserts an ID key with runtime override support.
pub fn insert_id_key_runtime<T: RegistryEntry + 'static>(
	store: &mut dyn KeyStore<T>,
	registry_label: &'static str,
	choose_winner: ChooseWinner<T>,
	id: &str,
	def: DefPtr<T>,
) -> Result<InsertAction, RegistryError> {
	let existing = store.get_id_owner(id);

	let Some(existing) = existing else {
		store.insert_id(id, def);
		return Ok(InsertAction::InsertedNew);
	};

	if existing.ptr_eq(def) {
		return Ok(InsertAction::KeptExisting);
	}

	let new_wins = choose_winner(KeyKind::Id, id, r(existing), r(def));
	let (action, winner_id) = if new_wins {
		store.evict_def(existing);
		store.set_id_owner(id, def);
		(InsertAction::ReplacedExisting, r(def).id())
	} else {
		(InsertAction::KeptExisting, r(existing).id())
	};

	store.push_collision(Collision {
		kind: KeyKind::Id,
		key: Box::from(id),
		existing_id: r(existing).id(),
		new_id: r(def).id(),
		winner_id,
		action,
		registry: registry_label,
	});

	Ok(action)
}
