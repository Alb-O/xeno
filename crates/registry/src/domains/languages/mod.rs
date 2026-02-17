//! Languages domain registration and runtime entry construction.

pub mod builtins;
pub mod link;
pub mod loader;
pub mod queries;
pub mod registry;
pub mod spec;
pub mod types;

pub use registry::LanguagesRegistry;
pub use types::{LanguageEntry, LanguageInput};

/// Registers compiled languages from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_languages_spec();
	let linked = link::link_languages(&spec);

	for def in linked {
		db.push_domain::<Languages>(LanguageInput::Linked(def));
	}
}

pub struct Languages;

impl crate::db::domain::DomainSpec for Languages {
	type Input = LanguageInput;
	type Entry = LanguageEntry;
	type Id = crate::core::LanguageId;
	type Runtime = LanguagesRegistry;
	const LABEL: &'static str = "languages";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.languages
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		LanguagesRegistry::new(index)
	}
}
