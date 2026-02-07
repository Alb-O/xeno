//! Runtime storage for option values.
//!
//! The [`OptionStore`] provides a container for option values that can be used
//! for global configuration, per-language settings, or buffer-local overrides.
//! Multiple stores can be combined using the [`OptionResolver`](crate::options::OptionResolver)
//! to implement layered configuration.

use std::collections::HashMap;

use crate::options::{OptionError, OptionKey, OptionValue};

#[cfg(test)]
mod tests;

/// Runtime storage for option values.
#[derive(Debug, Clone, Default)]
pub struct OptionStore {
	values: HashMap<String, OptionValue>,
}

impl OptionStore {
	/// Creates an empty store.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets an option value by typed key.
	pub fn set(&mut self, key: OptionKey, value: OptionValue) {
		self.values.insert(key.kdl_key.to_string(), value);
	}

	/// Sets an option value by KDL key (for config parsing).
	pub fn set_by_kdl(&mut self, kdl_key: &str, value: OptionValue) -> Result<(), OptionError> {
		if crate::db::OPTIONS.get(kdl_key).is_some() {
			self.values.insert(kdl_key.to_string(), value);
			return Ok(());
		}
		Err(OptionError::UnknownOption(kdl_key.to_string()))
	}

	/// Gets an option value, returning `None` if not set.
	pub fn get(&self, key: OptionKey) -> Option<&OptionValue> {
		self.values.get(key.kdl_key)
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
	pub fn remove(&mut self, key: OptionKey) -> Option<OptionValue> {
		self.values.remove(key.kdl_key)
	}

	/// Merges another store into this one.
	pub fn merge(&mut self, other: &OptionStore) {
		for (k, v) in &other.values {
			self.values.insert(k.clone(), v.clone());
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
	pub fn iter(&self) -> impl Iterator<Item = (&str, &OptionValue)> {
		self.values.iter().map(|(k, v)| (k.as_str(), v))
	}
}
