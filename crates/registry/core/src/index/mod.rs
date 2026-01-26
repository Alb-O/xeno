//! Centralized registry index infrastructure.

mod build;
mod collision;
mod mod_types;
mod runtime;

use std::cmp::Ordering;
use std::collections::HashMap;

pub use build::RegistryBuilder;
pub use collision::{
	ChooseWinner, Collision, DuplicatePolicy, InsertAction, InsertFatal, KeyKind, KeyStore,
	insert_typed_key,
};
pub use mod_types::{RegistryIndex, RegistryReg};
pub use runtime::RuntimeRegistry;

use crate::RegistryEntry;

#[cfg(test)]
mod tests;

/// Builds a secondary index map with custom keys.
pub fn build_map<T, K, F>(
	label: &'static str,
	items: &[&'static T],
	policy: DuplicatePolicy,
	mut key_of: F,
) -> HashMap<K, &'static T>
where
	T: RegistryEntry + 'static,
	K: Eq + std::hash::Hash + std::fmt::Debug,
	F: FnMut(&'static T) -> Option<K>,
{
	let mut map: HashMap<K, &'static T> = HashMap::with_capacity(items.len());

	for &item in items {
		let Some(key) = key_of(item) else { continue };

		if let Some(&existing) = map.get(&key) {
			if std::ptr::eq(existing, item) {
				continue;
			}
			match policy {
				DuplicatePolicy::Panic => {
					panic!("duplicate secondary key in {}: key={:?}", label, key)
				}
				DuplicatePolicy::FirstWins => {}
				DuplicatePolicy::LastWins => {
					map.insert(key, item);
				}
				DuplicatePolicy::ByPriority => {
					if item.total_order_cmp(existing) == Ordering::Greater {
						map.insert(key, item);
					}
				}
			}
		} else {
			map.insert(key, item);
		}
	}

	map
}
