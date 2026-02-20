pub struct Snippets;

impl crate::db::domain::DomainSpec for Snippets {
	type Input = super::SnippetInput;
	type Entry = super::SnippetEntry;
	type Id = crate::core::SnippetId;
	type Runtime = crate::core::RuntimeRegistry<super::SnippetEntry, crate::core::SnippetId>;
	const LABEL: &'static str = "snippets";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.snippets
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}
}
