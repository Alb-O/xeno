use super::collision::{ChooseWinner, Collision, KeyKind, KeyStore};
use crate::RegistryEntry;
use crate::error::{InsertAction, InsertFatal, RegistryError};

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

/// Inserts an ID key with runtime override support.
pub fn insert_id_key_runtime<T: RegistryEntry + 'static>(
	store: &mut dyn KeyStore<T>,
	registry_label: &'static str,
	choose_winner: ChooseWinner<T>,
	id: &'static str,
	def: &'static T,
) -> Result<InsertAction, RegistryError> {
	let existing = store.get_id_owner(id);

	let Some(existing) = existing else {
		store.insert_id(id, def);
		return Ok(InsertAction::InsertedNew);
	};

	if std::ptr::eq(existing, def) {
		return Ok(InsertAction::KeptExisting);
	}

	let new_wins = choose_winner(KeyKind::Id, id, existing, def);
	let (action, winner_id) = if new_wins {
		store.evict_def(existing);
		store.set_id_owner(id, def);
		(InsertAction::ReplacedExisting, def.id())
	} else {
		(InsertAction::KeptExisting, existing.id())
	};

	store.push_collision(Collision {
		kind: KeyKind::Id,
		key: id,
		existing_id: existing.id(),
		new_id: def.id(),
		winner_id,
		action,
		registry: registry_label,
	});

	Ok(action)
}
