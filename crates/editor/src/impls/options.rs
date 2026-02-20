//! Option resolution for the editor.
//!
//! Provides a single source of truth for resolving option values through the
//! layered configuration system.

use std::collections::HashMap;

use xeno_registry::options::{FromOptionValue, OptionKey, OptionResolver, OptionStore, OptionValue, TypedOptionKey};

use super::Editor;
use crate::buffer::ViewId;

impl Editor {
	/// Loads user config from the default config directory, logging diagnostics.
	pub fn load_user_config() -> Option<xeno_registry::config::Config> {
		let config_dir = crate::paths::get_config_dir()?;
		let report = xeno_registry::config::load::load_user_config_from_dir(&config_dir);

		for (path, warning) in &report.warnings {
			tracing::warn!(path = %path.display(), "{warning}");
		}

		for (path, error) in &report.errors {
			tracing::warn!(path = %path.display(), error = %error, "failed to load config");
		}

		report.config
	}

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
	/// use xeno_registry::options::option_keys;
	///
	/// let value = editor.resolve_option(buffer_id, keys::TAB_WIDTH.untyped());
	/// ```
	pub fn resolve_option(&self, buffer_id: ViewId, key: OptionKey) -> OptionValue {
		let opt = xeno_registry::db::OPTIONS.get_key(&key).expect("option key missing from registry");

		self.resolve_option_ref(buffer_id, &opt)
	}

	/// Resolves an option for a specific buffer using a resolved reference.
	pub fn resolve_option_ref(&self, buffer_id: ViewId, opt: &xeno_registry::options::OptionsRef) -> OptionValue {
		let buffer = self.state.core.editor.buffers.get_buffer(buffer_id).expect("buffer must exist");

		let language_store = buffer.file_type().and_then(|ft| self.state.config.config.language_options.get(&ft));

		Self::resolve_with_stores(&buffer.local_options, language_store, &self.state.config.config.global_options, opt)
	}

	/// Resolves a typed option for a specific buffer.
	///
	/// This is the preferred method for option access, providing compile-time
	/// type safety through [`TypedOptionKey<T>`].
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_registry::options::option_keys;
	///
	/// let width: i64 = editor.resolve_typed_option(buffer_id, keys::TAB_WIDTH);
	/// ```
	pub fn resolve_typed_option<T: FromOptionValue>(&self, buffer_id: ViewId, key: TypedOptionKey<T>) -> T {
		let opt = xeno_registry::db::OPTIONS
			.get_key(&key.untyped())
			.expect("typed option key missing from registry");

		T::from_option(&self.resolve_option_ref(buffer_id, &opt))
			.or_else(|| T::from_option(&opt.default.to_value()))
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
	/// use xeno_registry::options::option_keys;
	///
	/// let width: i64 = editor.option(keys::TAB_WIDTH);
	/// ```
	pub fn option<T: FromOptionValue>(&self, key: TypedOptionKey<T>) -> T {
		self.resolve_typed_option(self.focused_view(), key)
	}

	/// Replaces editor key/option configuration with a loaded user config.
	///
	/// This is used by startup and reload flows to keep config merge/apply
	/// behavior consistent across runtimes.
	pub fn apply_loaded_config(&mut self, mut config: Option<xeno_registry::config::Config>) {
		let mut key_overrides = None;
		let mut preset_name = None;
		let mut global_options = OptionStore::new();
		let mut language_options = HashMap::<String, OptionStore>::new();

		let mut nu_config = None;
		if let Some(mut loaded) = config.take() {
			if let Some(ref mut km) = loaded.keymap {
				key_overrides = km.keys.take();
				preset_name = km.preset.take();
			}
			nu_config = loaded.nu.take();
			global_options = loaded.options;

			for lang_config in loaded.languages {
				language_options.entry(lang_config.name).or_default().merge(&lang_config.options);
			}
		}

		self.set_key_overrides(key_overrides);
		self.set_keymap_preset(preset_name.unwrap_or_else(|| xeno_registry::keymaps::DEFAULT_PRESET.to_string()));
		let editor_config = self.config_mut();
		editor_config.global_options = global_options;
		editor_config.language_options = language_options;
		editor_config.nu = nu_config;
	}

	/// Internal helper that performs resolution given the stores directly.
	///
	/// This avoids borrowing issues when the buffer is already borrowed.
	fn resolve_with_stores(
		buffer_options: &OptionStore,
		language_options: Option<&OptionStore>,
		global_options: &OptionStore,
		opt: &xeno_registry::options::OptionsRef,
	) -> OptionValue {
		let resolver = if let Some(lang_store) = language_options {
			OptionResolver::new()
				.with_buffer(buffer_options)
				.with_language(lang_store)
				.with_global(global_options)
		} else {
			OptionResolver::new().with_buffer(buffer_options).with_global(global_options)
		};

		resolver.resolve(opt)
	}
}
