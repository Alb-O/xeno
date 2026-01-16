//! Option resolution for the editor.
//!
//! Provides a single source of truth for resolving option values through the
//! layered configuration system.

use xeno_registry::options::{
	FromOptionValue, OptionKey, OptionResolver, OptionStore, OptionValue, TypedOptionKey,
};

use super::Editor;
use crate::buffer::BufferId;

impl Editor {
	/// Resolves an option for a specific buffer through the full hierarchy.
	///
	/// Resolution order (highest priority first):
	/// 1. Buffer-local override (set via `:setlocal`)
	/// 2. Language-specific config (from `language "rust" { }` block)
	/// 3. Global config (from `options { }` block)
	/// 4. Compile-time default (from `#[derive_option]` macro)
	///
	/// # Panics
	///
	/// Panics if `buffer_id` does not refer to an existing buffer.
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_registry::options::keys;
	///
	/// let value = editor.resolve_option(buffer_id, keys::TAB_WIDTH.untyped());
	/// ```
	pub fn resolve_option(&self, buffer_id: BufferId, key: OptionKey) -> OptionValue {
		let buffer = self
			.buffers
			.get_buffer(buffer_id)
			.expect("buffer must exist");

		let language_store = buffer
			.file_type()
			.and_then(|ft| self.config.language_options.get(&ft));

		Self::resolve_with_stores(
			&buffer.local_options,
			language_store,
			&self.config.global_options,
			key,
		)
	}

	/// Resolves a typed option for a specific buffer.
	///
	/// This is the preferred method for option access, providing compile-time
	/// type safety through [`TypedOptionKey<T>`].
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_registry::options::keys;
	///
	/// let width: i64 = editor.resolve_typed_option(buffer_id, keys::TAB_WIDTH);
	/// ```
	pub fn resolve_typed_option<T: FromOptionValue>(
		&self,
		buffer_id: BufferId,
		key: TypedOptionKey<T>,
	) -> T {
		T::from_option(&self.resolve_option(buffer_id, key.untyped()))
			.or_else(|| T::from_option(&(key.def().default)()))
			.expect("option type mismatch with registered default")
	}

	/// Resolves a typed option for the currently focused buffer.
	///
	/// Convenience method that combines [`focused_view`](Self::focused_view) and
	/// [`resolve_typed_option`](Self::resolve_typed_option).
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_registry::options::keys;
	///
	/// let width: i64 = editor.option(keys::TAB_WIDTH);
	/// ```
	pub fn option<T: FromOptionValue>(&self, key: TypedOptionKey<T>) -> T {
		self.resolve_typed_option(self.focused_view(), key)
	}

	/// Internal helper that performs resolution given the stores directly.
	///
	/// This avoids borrowing issues when the buffer is already borrowed.
	fn resolve_with_stores(
		buffer_options: &OptionStore,
		language_options: Option<&OptionStore>,
		global_options: &OptionStore,
		key: OptionKey,
	) -> OptionValue {
		let resolver = if let Some(lang_store) = language_options {
			OptionResolver::new()
				.with_buffer(buffer_options)
				.with_language(lang_store)
				.with_global(global_options)
		} else {
			OptionResolver::new()
				.with_buffer(buffer_options)
				.with_global(global_options)
		};

		resolver.resolve(key)
	}
}
