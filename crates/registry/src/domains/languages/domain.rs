pub struct Languages;

impl crate::db::domain::DomainSpec for Languages {
	type Input = super::LanguageInput;
	type Entry = super::LanguageEntry;
	type Id = crate::core::LanguageId;
	type Runtime = super::LanguagesRegistry;
	const LABEL: &'static str = "languages";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.languages
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		super::LanguagesRegistry::new(index)
	}
}
