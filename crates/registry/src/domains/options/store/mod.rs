//! Runtime storage for option values.
//!
//! The [`OptionStore`] provides a container for option values that can be used
//! for global configuration, per-language settings, or buffer-local overrides.
//! Multiple stores can be combined using the [`OptionResolver`](crate::options::OptionResolver)
//! to implement layered configuration.

use crate::core::{DenseId, OptionId};
use crate::options::{OptionError, OptionValue, OptionsRef, OptionsRegistry};

#[cfg(test)]
mod tests;

/// Runtime storage for option values using dense-ID indexing.
#[derive(Debug, Clone, Default)]
pub struct OptionStore {
	values: Vec<Option<OptionValue>>,
}

impl OptionStore {
	/// Creates an empty store.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a store sized for the given registry.
	pub fn with_capacity(reg: &OptionsRegistry) -> Self {
		Self { values: vec![None; reg.len()] }
	}

	fn ensure_len(&mut self, id: OptionId) {
		let idx = id.as_u32() as usize;
		if idx >= self.values.len() {
			self.values.resize_with(idx + 1, || None);
		}
	}

	/// Sets an option value by reference.
	pub fn set(&mut self, opt: OptionsRef, value: OptionValue) {
		let id = opt.dense_id();
		self.ensure_len(id);
		self.values[id.as_u32() as usize] = Some(value);
	}

	/// Sets an option value by KDL key (for config parsing).
	pub fn set_by_kdl(&mut self, reg: &OptionsRegistry, kdl_key: &str, value: OptionValue) -> Result<(), OptionError> {
		let opt = reg.get(kdl_key).ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;

		crate::options::validate_ref(&opt, &value)?;

		self.set(opt, value);
		Ok(())
	}

	/// Gets an option value, returning `None` if not set.
	pub fn get(&self, id: OptionId) -> Option<&OptionValue> {
		self.values.get(id.as_u32() as usize)?.as_ref()
	}

	/// Gets typed value with automatic conversion to `i64`.
	pub fn get_int(&self, id: OptionId) -> Option<i64> {
		self.get(id).and_then(|v| v.as_int())
	}

	/// Gets typed value with automatic conversion to `bool`.
	pub fn get_bool(&self, id: OptionId) -> Option<bool> {
		self.get(id).and_then(|v| v.as_bool())
	}

	/// Gets typed value with automatic conversion to `&str`.
	pub fn get_string(&self, id: OptionId) -> Option<&str> {
		self.get(id).and_then(|v| v.as_str())
	}

	/// Removes an option from the store.
	pub fn remove(&mut self, opt: OptionsRef) -> Option<OptionValue> {
		self.values.get_mut(opt.dense_id().as_u32() as usize)?.take()
	}

	/// Merges another store into this one.
	pub fn merge(&mut self, other: &OptionStore) {
		if other.values.len() > self.values.len() {
			self.values.resize_with(other.values.len(), || None);
		}
		for (i, v) in other.values.iter().enumerate() {
			if let Some(v) = v {
				self.values[i] = Some(v.clone());
			}
		}
	}

	/// Returns the number of options set in this store.
	pub fn len(&self) -> usize {
		self.values.iter().filter(|v| v.is_some()).count()
	}

	/// Returns `true` if the store contains no set options.
	pub fn is_empty(&self) -> bool {
		self.values.iter().all(|v| v.is_none())
	}

	/// Returns an iterator over all set values.
	pub fn iter(&self) -> impl Iterator<Item = (OptionId, &OptionValue)> {
		self.values
			.iter()
			.enumerate()
			.filter_map(|(i, v)| v.as_ref().map(|val| (OptionId::from_u32(i as u32), val)))
	}
}
