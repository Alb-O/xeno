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

use crate::options::{OptionKey, OptionStore, OptionValue};

#[cfg(test)]
mod tests;

/// Resolves option values through a layered hierarchy.
#[derive(Default)]
pub struct OptionResolver<'a> {
	buffer_local: Option<&'a OptionStore>,
	language: Option<&'a OptionStore>,
	global: Option<&'a OptionStore>,
}

impl<'a> OptionResolver<'a> {
	/// Creates a new resolver with no stores configured.
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds a buffer-local store (highest priority).
	pub fn with_buffer(mut self, store: &'a OptionStore) -> Self {
		self.buffer_local = Some(store);
		self
	}

	/// Adds a language-specific store.
	pub fn with_language(mut self, store: &'a OptionStore) -> Self {
		self.language = Some(store);
		self
	}

	/// Adds a global configuration store.
	pub fn with_global(mut self, store: &'a OptionStore) -> Self {
		self.global = Some(store);
		self
	}

	/// Resolves an option through the hierarchy.
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
		key.default.to_value()
	}

	/// Resolves an integer option through the hierarchy.
	pub fn resolve_int(&self, key: OptionKey) -> i64 {
		let resolved = self.resolve(key);
		if let Some(v) = resolved.as_int() {
			return v;
		}

		let def = key;
		tracing::warn!(
			domain = "options",
			expected = "int",
			got = resolved.type_name(),
			"option type mismatch; falling back to default",
		);

		match def.default {
			crate::options::OptionDefault::Int(f) => f(),
			_ => unreachable!("validated in RegistryDbBuilder::register_option"),
		}
	}

	/// Resolves a boolean option through the hierarchy.
	pub fn resolve_bool(&self, key: OptionKey) -> bool {
		let resolved = self.resolve(key);
		if let Some(v) = resolved.as_bool() {
			return v;
		}

		let def = key;
		tracing::warn!(
			domain = "options",
			expected = "bool",
			got = resolved.type_name(),
			"option type mismatch; falling back to default",
		);

		match def.default {
			crate::options::OptionDefault::Bool(f) => f(),
			_ => unreachable!("validated in RegistryDbBuilder::register_option"),
		}
	}

	/// Resolves a string option through the hierarchy.
	pub fn resolve_string(&self, key: OptionKey) -> String {
		let resolved = self.resolve(key);
		if let Some(v) = resolved.as_str() {
			return v.to_string();
		}

		let def = key;
		tracing::warn!(
			domain = "options",
			expected = "string",
			got = resolved.type_name(),
			"option type mismatch; falling back to default",
		);

		match def.default {
			crate::options::OptionDefault::String(f) => f(),
			_ => unreachable!("validated in RegistryDbBuilder::register_option"),
		}
	}
}
