//! Runtime storage for option values.
//!
//! The [`OptionStore`] provides a container for option values that can be used
//! for global configuration, per-language settings, or buffer-local overrides.
//! Multiple stores can be combined using the [`OptionResolver`](crate::OptionResolver)
//! to implement layered configuration.

use std::collections::HashMap;

use crate::{find_by_kdl, OptionError, OptionKey, OptionValue};

/// Runtime storage for option values.
///
/// An `OptionStore` holds a collection of option values keyed by their KDL key.
/// Multiple stores can be combined using the [`OptionResolver`](crate::OptionResolver)
/// to implement layered configuration.
///
/// # Example
///
/// ```ignore
/// use xeno_registry_options::{keys, OptionStore, OptionValue};
///
/// let mut store = OptionStore::new();
/// store.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(2));
///
/// assert_eq!(store.get_int(keys::TAB_WIDTH.untyped()), Some(2));
/// ```
#[derive(Debug, Clone, Default)]
pub struct OptionStore {
	/// Values keyed by KDL key for config parsing.
	values: HashMap<&'static str, OptionValue>,
}

impl OptionStore {
	/// Creates an empty store.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets an option value by typed key.
	pub fn set(&mut self, key: OptionKey, value: OptionValue) {
		self.values.insert(key.def().kdl_key, value);
	}

	/// Sets an option value by KDL key (for config parsing).
	///
	/// Returns an error if the KDL key is not recognized.
	pub fn set_by_kdl(&mut self, kdl_key: &str, value: OptionValue) -> Result<(), OptionError> {
		let def =
			find_by_kdl(kdl_key).ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;
		self.values.insert(def.kdl_key, value);
		Ok(())
	}

	/// Gets an option value, returning `None` if not set.
	pub fn get(&self, key: OptionKey) -> Option<&OptionValue> {
		self.values.get(key.def().kdl_key)
	}

	/// Gets typed value with automatic conversion to `i64`.
	pub fn get_int(&self, key: OptionKey) -> Option<i64> {
		self.get(key).and_then(|v| v.as_int())
	}

	/// Gets typed value with automatic conversion to `bool`.
	pub fn get_bool(&self, key: OptionKey) -> Option<bool> {
		self.get(key).and_then(|v| v.as_bool())
	}

	/// Gets typed value with automatic conversion to `&str`.
	pub fn get_string(&self, key: OptionKey) -> Option<&str> {
		self.get(key).and_then(|v| v.as_str())
	}

	/// Removes an option from the store.
	///
	/// Returns the previous value if it existed.
	pub fn remove(&mut self, key: OptionKey) -> Option<OptionValue> {
		self.values.remove(key.def().kdl_key)
	}

	/// Merges another store into this one.
	///
	/// Values from `other` take precedence on conflict.
	pub fn merge(&mut self, other: &OptionStore) {
		for (k, v) in &other.values {
			self.values.insert(k, v.clone());
		}
	}

	/// Returns the number of options in this store.
	pub fn len(&self) -> usize {
		self.values.len()
	}

	/// Returns `true` if the store contains no options.
	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}

	/// Returns an iterator over all set values.
	///
	/// Yields tuples of (KDL key, value).
	pub fn iter(&self) -> impl Iterator<Item = (&'static str, &OptionValue)> {
		self.values.iter().map(|(k, v)| (*k, v))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::keys;

	#[test]
	fn test_set_and_get() {
		let mut store = OptionStore::new();
		store.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(8));

		assert_eq!(store.get(keys::TAB_WIDTH.untyped()), Some(&OptionValue::Int(8)));
		assert_eq!(store.get_int(keys::TAB_WIDTH.untyped()), Some(8));
	}

	#[test]
	fn test_get_missing() {
		let store = OptionStore::new();
		assert_eq!(store.get(keys::TAB_WIDTH.untyped()), None);
		assert_eq!(store.get_int(keys::TAB_WIDTH.untyped()), None);
	}

	#[test]
	fn test_set_by_kdl() {
		let mut store = OptionStore::new();
		store.set_by_kdl("tab-width", OptionValue::Int(2)).unwrap();

		assert_eq!(store.get_int(keys::TAB_WIDTH.untyped()), Some(2));
	}

	#[test]
	fn test_set_by_kdl_unknown() {
		let mut store = OptionStore::new();
		let result = store.set_by_kdl("unknown-option", OptionValue::Int(1));

		assert!(matches!(result, Err(OptionError::UnknownOption(_))));
	}

	#[test]
	fn test_merge() {
		let mut store1 = OptionStore::new();
		store1.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));
		store1.set(keys::THEME.untyped(), OptionValue::String("gruvbox".to_string()));

		let mut store2 = OptionStore::new();
		store2.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(2));

		store1.merge(&store2);

		// store2's value wins
		assert_eq!(store1.get_int(keys::TAB_WIDTH.untyped()), Some(2));
		// store1's value preserved
		assert_eq!(store1.get_string(keys::THEME.untyped()), Some("gruvbox"));
	}

	#[test]
	fn test_remove() {
		let mut store = OptionStore::new();
		store.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));

		let removed = store.remove(keys::TAB_WIDTH.untyped());
		assert_eq!(removed, Some(OptionValue::Int(4)));
		assert_eq!(store.get(keys::TAB_WIDTH.untyped()), None);
	}

	#[test]
	fn test_len_and_is_empty() {
		let mut store = OptionStore::new();
		assert!(store.is_empty());
		assert_eq!(store.len(), 0);

		store.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));
		assert!(!store.is_empty());
		assert_eq!(store.len(), 1);
	}

	#[test]
	fn test_string_option() {
		let mut store = OptionStore::new();
		store.set(keys::THEME.untyped(), OptionValue::String("monokai".to_string()));

		assert_eq!(store.get_string(keys::THEME.untyped()), Some("monokai"));
	}
}
