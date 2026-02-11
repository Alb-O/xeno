//! Snippet registry.

pub mod builtins;
pub mod def;
pub mod entry;
pub mod link;
pub mod loader;
pub mod spec;

pub use builtins::register_builtins;
pub use def::{SnippetDef, SnippetInput};
pub use entry::SnippetEntry;
pub use link::LinkedSnippetDef;

pub fn register_plugin(db: &mut crate::db::builder::RegistryDbBuilder) -> Result<(), crate::error::RegistryError> {
	register_compiled(db);
	Ok(())
}

/// Registers compiled snippets from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_snippets_spec();
	let linked = link::link_snippets(&spec);

	for def in linked {
		db.push_domain::<Snippets>(SnippetInput::Linked(def));
	}
}

pub struct Snippets;

impl crate::db::domain::DomainSpec for Snippets {
	type Input = SnippetInput;
	type Entry = SnippetEntry;
	type Id = crate::core::SnippetId;
	type StaticDef = SnippetDef;
	type LinkedDef = LinkedSnippetDef;
	const LABEL: &'static str = "snippets";

	fn static_to_input(def: &'static Self::StaticDef) -> Self::Input {
		SnippetInput::Static(def.clone())
	}

	fn linked_to_input(def: Self::LinkedDef) -> Self::Input {
		SnippetInput::Linked(def)
	}

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.snippets
	}
}

pub type SnippetRef = crate::core::RegistryRef<SnippetEntry, crate::core::SnippetId>;

#[cfg(feature = "db")]
pub use crate::db::SNIPPETS;

pub fn find_snippet(key: &str) -> Option<SnippetRef> {
	let key = normalize_lookup_key(key);
	if key.is_empty() {
		return None;
	}

	#[cfg(feature = "db")]
	{
		return SNIPPETS.get(key);
	}

	#[cfg(not(feature = "db"))]
	{
		let _ = key;
		None
	}
}

#[cfg(feature = "db")]
pub fn all_snippets() -> Vec<SnippetRef> {
	SNIPPETS.snapshot_guard().iter_refs().collect()
}

fn normalize_lookup_key(key: &str) -> &str {
	key.strip_prefix('@').unwrap_or(key)
}

#[cfg(all(test, feature = "db"))]
mod tests {
	use super::{all_snippets, find_snippet};

	#[test]
	fn find_snippet_resolves_by_name_and_key_with_optional_at_prefix() {
		let all = all_snippets();
		assert!(!all.is_empty(), "snippets registry should contain compiled snippets");

		assert!(find_snippet("fori").is_some());
		assert!(find_snippet("@fori").is_some());
		assert!(find_snippet("forloop").is_some());
		assert!(find_snippet("@forloop").is_some());
	}
}
