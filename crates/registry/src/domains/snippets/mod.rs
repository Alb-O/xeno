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
	const LABEL: &'static str = "snippets";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.snippets
	}
}

pub type SnippetRef = crate::core::RegistryRef<SnippetEntry, crate::core::SnippetId>;

#[cfg(feature = "minimal")]
pub use crate::db::SNIPPETS;

pub fn find_snippet(key: &str) -> Option<SnippetRef> {
	let key = normalize_lookup_key(key);
	if key.is_empty() {
		return None;
	}

	#[cfg(feature = "minimal")]
	{
		SNIPPETS.get(key)
	}

	#[cfg(not(feature = "minimal"))]
	{
		let _ = key;
		None
	}
}

#[cfg(feature = "minimal")]
pub fn all_snippets() -> Vec<SnippetRef> {
	SNIPPETS.snapshot_guard().iter_refs().collect()
}

fn normalize_lookup_key(key: &str) -> &str {
	key.strip_prefix('@').unwrap_or(key)
}

#[cfg(all(test, feature = "minimal"))]
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
