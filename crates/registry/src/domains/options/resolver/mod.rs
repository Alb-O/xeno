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

use crate::options::{OptionStore, OptionValue, OptionsRef};

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
	pub fn resolve(&self, opt: &OptionsRef) -> OptionValue {
		if let Some(store) = self.buffer_local
			&& let Some(v) = store.get(opt.dense_id())
		{
			return v.clone();
		}
		if let Some(store) = self.language
			&& let Some(v) = store.get(opt.dense_id())
		{
			return v.clone();
		}
		if let Some(store) = self.global
			&& let Some(v) = store.get(opt.dense_id())
		{
			return v.clone();
		}
		opt.default.to_value()
	}

	/// Resolves an integer option through the hierarchy.
	pub fn resolve_int(&self, opt: &OptionsRef) -> i64 {
		let resolved = self.resolve(opt);
		if let Some(v) = resolved.as_int() {
			return v;
		}

		tracing::warn!(
			domain = "options",
			expected = "int",
			got = resolved.type_name(),
			"option type mismatch; falling back to default",
		);

		opt.default.to_value().as_int().expect("validated at build")
	}

	/// Resolves a boolean option through the hierarchy.
	pub fn resolve_bool(&self, opt: &OptionsRef) -> bool {
		let resolved = self.resolve(opt);
		if let Some(v) = resolved.as_bool() {
			return v;
		}

		tracing::warn!(
			domain = "options",
			expected = "bool",
			got = resolved.type_name(),
			"option type mismatch; falling back to default",
		);

		opt.default.to_value().as_bool().expect("validated at build")
	}

	/// Resolves a string option through the hierarchy.
	pub fn resolve_string(&self, opt: &OptionsRef) -> String {
		let resolved = self.resolve(opt);
		if let Some(v) = resolved.as_str() {
			return v.to_string();
		}

		tracing::warn!(
			domain = "options",
			expected = "string",
			got = resolved.type_name(),
			"option type mismatch; falling back to default",
		);

		opt.default.to_value().as_str().expect("validated at build").to_string()
	}
}
