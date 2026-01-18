//! Layered option resolution.
//!
//! The [`OptionResolver`] provides a way to resolve option values through a
//! hierarchy of configuration layers, from most specific (buffer-local) to
//! least specific (compile-time defaults).
//!
//! # Resolution Order
//!
//! 1. Buffer-local override (set via `:setlocal`)
//! 2. Language-specific config (from `language "rust" { }` block)
//! 3. Global config (from `options { }` block)
//! 4. Compile-time default (from `#[derive_option]` macro)

use crate::{OptionKey, OptionStore, OptionValue};

#[cfg(test)]
mod tests;

/// Resolves option values through a layered hierarchy.
///
/// The resolver is stateless and created per-resolution. It chains multiple
/// [`OptionStore`]s in priority order and falls back to the option's default
/// value if not found in any store.
///
/// # Example
///
/// ```ignore
/// use xeno_registry_options::{keys, OptionResolver, OptionStore, OptionValue};
///
/// let mut global = OptionStore::new();
/// global.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(4));
///
/// let mut buffer = OptionStore::new();
/// buffer.set(keys::TAB_WIDTH.untyped(), OptionValue::Int(2));
///
/// let resolver = OptionResolver::new()
///     .with_global(&global)
///     .with_buffer(&buffer);
///
/// // Buffer-local wins
/// assert_eq!(resolver.resolve(keys::TAB_WIDTH.untyped()).as_int(), Some(2));
/// ```
#[derive(Default)]
pub struct OptionResolver<'a> {
	buffer_local: Option<&'a OptionStore>,
	language: Option<&'a OptionStore>,
	global: Option<&'a OptionStore>,
}

impl<'a> OptionResolver<'a> {
	/// Creates a new resolver with no stores configured.
	///
	/// All resolutions will fall back to the option's default value until
	/// stores are added via the builder methods.
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds a buffer-local store (highest priority).
	///
	/// Values in this store take precedence over all other layers.
	pub fn with_buffer(mut self, store: &'a OptionStore) -> Self {
		self.buffer_local = Some(store);
		self
	}

	/// Adds a language-specific store.
	///
	/// Values in this store take precedence over global config but
	/// are overridden by buffer-local settings.
	pub fn with_language(mut self, store: &'a OptionStore) -> Self {
		self.language = Some(store);
		self
	}

	/// Adds a global configuration store.
	///
	/// Values in this store are used when not overridden by language
	/// or buffer-local settings.
	pub fn with_global(mut self, store: &'a OptionStore) -> Self {
		self.global = Some(store);
		self
	}

	/// Resolves an option through the hierarchy.
	///
	/// Checks each layer in order: buffer-local -> language -> global -> default.
	/// Returns the first found value, or the option's compile-time default.
	pub fn resolve(&self, key: OptionKey) -> OptionValue {
		if let Some(store) = self.buffer_local
			&& let Some(v) = store.get(key)
		{
			return v.clone();
		}
		if let Some(store) = self.language
			&& let Some(v) = store.get(key)
		{
			return v.clone();
		}
		if let Some(store) = self.global
			&& let Some(v) = store.get(key)
		{
			return v.clone();
		}
		(key.def().default)()
	}

	/// Resolves an integer option through the hierarchy.
	///
	/// If the resolved value is not an integer (type mismatch), falls back
	/// to the option's default value.
	pub fn resolve_int(&self, key: OptionKey) -> i64 {
		self.resolve(key)
			.as_int()
			.unwrap_or_else(|| (key.def().default)().as_int().unwrap())
	}

	/// Resolves a boolean option through the hierarchy.
	///
	/// If the resolved value is not a boolean (type mismatch), falls back
	/// to the option's default value.
	pub fn resolve_bool(&self, key: OptionKey) -> bool {
		self.resolve(key)
			.as_bool()
			.unwrap_or_else(|| (key.def().default)().as_bool().unwrap())
	}

	/// Resolves a string option through the hierarchy.
	///
	/// If the resolved value is not a string (type mismatch), falls back
	/// to the option's default value.
	pub fn resolve_string(&self, key: OptionKey) -> String {
		self.resolve(key)
			.as_str()
			.map(|s| s.to_string())
			.unwrap_or_else(|| (key.def().default)().as_str().unwrap().to_string())
	}
}
