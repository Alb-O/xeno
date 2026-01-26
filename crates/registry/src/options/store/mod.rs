//! Runtime storage for option values.
//!
//! The [`OptionStore`] provides a container for option values that can be used
//! for global configuration, per-language settings, or buffer-local overrides.
//! Multiple stores can be combined using the [`OptionResolver`](crate::options::OptionResolver)
//! to implement layered configuration.

use std::collections::HashMap;

use crate::options::{OptionError, OptionKey, OptionValue, find_by_kdl};

#[cfg(test)]
mod tests;

/// Runtime storage for option values.
///
/// An `OptionStore` holds a collection of option values keyed by their KDL key.
/// Multiple stores can be combined using the [`OptionResolver`](crate::options::OptionResolver)
/// to implement layered configuration.
///
/// # Example
///
/// ```ignore
/// use crate::options::{keys, OptionStore, OptionValue};
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
