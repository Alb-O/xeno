//! Language loader and registry.
//!
//! The [`LanguageLoader`] is a thin wrapper over [`LanguageDb`] that implements
//! [`tree_house::LanguageLoader`] for injection handling.
//!
//! [`LanguageDb`]: crate::db::LanguageDb

use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

pub use tree_house::Language as LanguageId;
use tree_house::{InjectionLanguageMarker, Language, LanguageConfig as TreeHouseConfig};
use xeno_registry::db::LANGUAGES;
use xeno_registry::languages::registry::LanguageRef;

use crate::db::{LanguageDb, language_db};
use crate::ids::{RegistryLanguageIdExt, TreeHouseLanguageExt};
use crate::language::LanguageData;

/// Wrapper over [`LanguageDb`] implementing `tree_house::LanguageLoader`.
///
/// Use [`from_embedded()`](Self::from_embedded) to create a loader backed by
/// the global language database (backed by the registry).
#[derive(Debug, Clone)]
pub struct LanguageLoader {
	db: Arc<LanguageDb>,
}

impl Default for LanguageLoader {
	fn default() -> Self {
		Self::from_embedded()
	}
}

impl LanguageLoader {
	/// Creates an empty loader.
	pub fn new() -> Self {
		Self {
			db: Arc::new(LanguageDb::new()),
		}
	}

	/// Creates a loader backed by the global language database.
	pub fn from_embedded() -> Self {
		Self {
			db: Arc::clone(language_db()),
		}
	}

	/// Creates a loader from a custom database.
	pub fn from_db(db: Arc<LanguageDb>) -> Self {
		Self { db }
	}

	/// Gets language data by ID.
	pub fn get(&self, lang: Language) -> Option<LanguageData> {
		self.db.get(lang.idx())
	}

	/// Finds a language by name.
	pub fn language_for_name(&self, name: &str) -> Option<Language> {
		LANGUAGES
			.get(name)
			.map(|r: LanguageRef| r.dense_id().to_tree_house())
	}

	/// Finds a language by file path.
	pub fn language_for_path(&self, path: &Path) -> Option<Language> {
		LANGUAGES
			.resolve_path(path)
			.map(|r: LanguageRef| r.dense_id().to_tree_house())
	}

	/// Finds a language by shebang line.
	pub fn language_for_shebang(&self, first_line: &str) -> Option<Language> {
		if !first_line.starts_with("#!") {
			return None;
		}

		let line = first_line.trim_start_matches("#!");
		let parts: Vec<&str> = line.split_whitespace().collect();

		let interpreter = if parts.first() == Some(&"/usr/bin/env") || parts.first() == Some(&"env")
		{
			parts.get(1).copied()
		} else {
			parts.first().and_then(|p| p.rsplit('/').next())
		};

		interpreter.and_then(|interp| {
			let base = interp.trim_end_matches(|c: char| c.is_ascii_digit());
			LANGUAGES
				.language_for_shebang(base)
				.map(|r: LanguageRef| r.dense_id().to_tree_house())
		})
	}

	/// Finds a language by matching text against injection regexes.
	fn language_for_injection_match(&self, text: &str) -> Option<Language> {
		LANGUAGES.all().into_iter().find_map(|l: LanguageRef| {
			let data = LanguageData { entry: l };
			data.injection_regex()
				.filter(|r| r.is_match(text))
				.map(|_| data.entry.dense_id().to_tree_house())
		})
	}

	/// Returns all registered languages.
	pub fn languages(&self) -> impl Iterator<Item = (Language, LanguageData)> {
		LANGUAGES
			.all()
			.into_iter()
			.map(|l: LanguageRef| (l.dense_id().to_tree_house(), LanguageData { entry: l }))
	}

	/// Returns the number of registered languages.
	pub fn len(&self) -> usize {
		LANGUAGES.len()
	}

	/// Returns true if no languages are registered.
	pub fn is_empty(&self) -> bool {
		LANGUAGES.is_empty()
	}

	/// Returns a loader view with the specified injection policy.
	pub fn with_injections(&self, injections: bool) -> LoaderView<'_> {
		LoaderView {
			base: self,
			injections,
		}
	}
}

/// A view of a [`LanguageLoader`] with a specific injection policy.
pub struct LoaderView<'a> {
	base: &'a LanguageLoader,
	injections: bool,
}

impl tree_house::LanguageLoader for LoaderView<'_> {
	fn language_for_marker(&self, marker: InjectionLanguageMarker) -> Option<Language> {
		if !self.injections {
			return None;
		}
		self.base.language_for_marker(marker)
	}

	fn get_config(&self, lang: Language) -> Option<&TreeHouseConfig> {
		self.base.get_config(lang)
	}
}

impl tree_house::LanguageLoader for LanguageLoader {
	fn language_for_marker(&self, marker: InjectionLanguageMarker) -> Option<Language> {
		match marker {
			InjectionLanguageMarker::Name(name) => self.language_for_name(name),
			InjectionLanguageMarker::Match(text) => {
				self.language_for_injection_match(&Cow::<str>::from(text))
			}
			InjectionLanguageMarker::Filename(text) => {
				self.language_for_path(Path::new(Cow::<str>::from(text).as_ref()))
			}
			InjectionLanguageMarker::Shebang(text) => {
				self.language_for_shebang(&Cow::<str>::from(text))
			}
		}
	}

	fn get_config(&self, lang: Language) -> Option<&TreeHouseConfig> {
		let max = LANGUAGES.len_u32();
		let id = lang.to_registry(max)?;
		self.db.get_config(id)
	}
}
