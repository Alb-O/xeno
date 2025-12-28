//! Configuration system for Evildoer.
//!
//! This crate provides unified configuration parsing and loading for the editor.
//! Configuration is written in KDL (v2) format and supports:
//!
//! - **Themes**: Color schemes for UI and syntax highlighting
//! - **Keybindings**: Key-to-action mappings per mode
//! - **Options**: Editor settings (indent, scrolloff, etc.)
//! - **Languages**: File type detection and syntax configuration
//!
//! # Configuration Files
//!
//! Evildoer looks for configuration in these locations (in order):
//!
//! 1. `$XDG_CONFIG_HOME/evildoer/config.kdl` (or `~/.config/evildoer/config.kdl`)
//! 2. `$XDG_CONFIG_HOME/evildoer/themes/*.kdl` - Additional theme files
//! 3. Runtime defaults bundled with the editor
//!
//! # Unified Config Schema
//!
//! A single config file can contain multiple sections:
//!
//! ```kdl
//! // Theme configuration
//! theme {
//!     name "my-theme"
//!     variant "dark"
//!     
//!     palette {
//!         primary "#ff0000"
//!     }
//!     
//!     ui {
//!         bg "#1a1a1a"
//!         fg "#ffffff"
//!         // ...
//!     }
//!     
//!     syntax {
//!         keyword fg="$primary"
//!     }
//! }
//!
//! // Keybinding overrides
//! keys {
//!     normal {
//!         "ctrl+s" "write"
//!         "ctrl+q" "quit"
//!     }
//!     insert {
//!         "ctrl+c" "normal_mode"
//!     }
//! }
//!
//! // Option overrides
//! options {
//!     tab-width 4
//!     indent-width 4
//!     use-tabs #false
//!     scrolloff 5
//!     theme "gruvbox"
//! }
//!
//! // Language-specific settings
//! language "rust" {
//!     tab-width 4
//!     indent-width 4
//! }
//!
//! language "python" {
//!     tab-width 4
//!     indent-width 4
//!     use-tabs #false
//! }
//! ```

pub mod error;
pub mod kdl_util;
pub mod keys;
pub mod options;
pub mod theme;

use std::path::Path;

pub use error::{ConfigError, Result};
pub use keys::KeysConfig;
pub use options::OptionsConfig;
pub use theme::ParsedTheme;

/// Parsed configuration from a KDL file.
///
/// May contain any combination of theme, keys, options, and language settings.
#[derive(Debug, Clone, Default)]
pub struct Config {
	pub theme: Option<ParsedTheme>,
	/// Keybinding overrides.
	pub keys: Option<KeysConfig>,
	/// Option overrides.
	pub options: Option<OptionsConfig>,
	/// Per-language option overrides.
	pub languages: Vec<LanguageConfig>,
}

/// Per-language configuration overrides.
#[derive(Debug, Clone)]
pub struct LanguageConfig {
	/// Language name (e.g., "rust", "python").
	pub name: String,
	/// Option overrides for this language.
	pub options: OptionsConfig,
}

impl Config {
	/// Parse a KDL string into a [`Config`].
	pub fn parse(input: &str) -> Result<Self> {
		let doc: kdl::KdlDocument = input.parse()?;

		let theme = doc.get("theme").map(theme::parse_theme_node).transpose()?;
		let keys = doc.get("keys").map(keys::parse_keys_node).transpose()?;
		let options = doc
			.get("options")
			.map(options::parse_options_node)
			.transpose()?;

		let languages = doc
			.nodes()
			.iter()
			.filter(|n| n.name().value() == "language")
			.filter_map(|node| {
				let name = node.get(0).and_then(|v| v.as_string())?.to_string();
				let options = options::parse_options_from_children(node).ok()?;
				Some(LanguageConfig { name, options })
			})
			.collect();

		Ok(Config {
			theme,
			keys,
			options,
			languages,
		})
	}

	/// Load configuration from a file.
	pub fn load(path: impl AsRef<Path>) -> Result<Self> {
		let path = path.as_ref();
		let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
			path: path.to_path_buf(),
			error: e,
		})?;
		Self::parse(&content)
	}

	/// Merge another config into this one.
	///
	/// Values from `other` override values in `self`.
	pub fn merge(&mut self, other: Config) {
		if other.theme.is_some() {
			self.theme = other.theme;
		}
		if let Some(other_keys) = other.keys {
			match &mut self.keys {
				Some(keys) => keys.merge(other_keys),
				None => self.keys = Some(other_keys),
			}
		}
		if let Some(other_opts) = other.options {
			match &mut self.options {
				Some(opts) => opts.merge(other_opts),
				None => self.options = Some(other_opts),
			}
		}
		self.languages.extend(other.languages);
	}
}

/// Load a standalone theme file.
///
/// Theme files use the same schema as the `theme { }` block in config.kdl,
/// but at the top level.
pub fn load_theme_file(path: impl AsRef<Path>) -> Result<ParsedTheme> {
	let path = path.as_ref();
	let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
		path: path.to_path_buf(),
		error: e,
	})?;
	let mut theme = theme::parse_standalone_theme(&content)?;
	theme.source_path = Some(path.to_path_buf());
	Ok(theme)
}

/// Load all theme files from a directory.
pub fn load_themes_from_directory(dir: impl AsRef<Path>) -> Result<Vec<ParsedTheme>> {
	let dir = dir.as_ref();
	let mut themes = Vec::new();

	let entries = std::fs::read_dir(dir).map_err(|e| ConfigError::Io {
		path: dir.to_path_buf(),
		error: e,
	})?;

	for entry in entries.flatten() {
		let path = entry.path();
		if path.extension().is_some_and(|ext| ext == "kdl") {
			match load_theme_file(&path) {
				Ok(theme) => themes.push(theme),
				Err(e) => {
					eprintln!("Warning: failed to load theme {:?}: {}", path, e);
				}
			}
		}
	}

	Ok(themes)
}

/// Load themes from a directory and register them in the runtime theme registry.
/// This should be called once at startup.
pub fn load_and_register_themes(dir: impl AsRef<Path>) -> Result<()> {
	let themes = load_themes_from_directory(dir)?;
	let owned: Vec<_> = themes.into_iter().map(|t| t.into_owned_theme()).collect();
	evildoer_manifest::register_runtime_themes(owned);
	Ok(())
}
