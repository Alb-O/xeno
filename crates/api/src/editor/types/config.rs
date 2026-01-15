//! Editor configuration.

use std::collections::HashMap;

use xeno_registry::options::OptionStore;
use xeno_registry::themes::Theme;
use xeno_runtime_language::LanguageLoader;

/// Editor configuration.
///
/// Groups static configuration that rarely changes after initialization:
/// theme, language settings, and user options.
pub struct Config {
	/// Current theme.
	pub theme: &'static Theme,
	/// Language configuration loader.
	pub language_loader: LanguageLoader,
	/// Global user configuration options.
	pub global_options: OptionStore,
	/// Per-language option overrides.
	pub language_options: HashMap<String, OptionStore>,
}

impl Config {
	/// Creates a new config with default theme and empty options.
	pub fn new(language_loader: LanguageLoader) -> Self {
		Self {
			theme: xeno_registry::themes::get_theme(xeno_registry::themes::DEFAULT_THEME_ID)
				.unwrap_or(&xeno_registry::themes::DEFAULT_THEME),
			language_loader,
			global_options: OptionStore::new(),
			language_options: HashMap::new(),
		}
	}
}
