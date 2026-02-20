pub struct Themes;

impl crate::db::domain::DomainSpec for Themes {
	type Input = super::ThemeInput;
	type Entry = super::ThemeEntry;
	type Id = crate::core::ThemeId;
	type Runtime = crate::core::RuntimeRegistry<super::ThemeEntry, crate::core::ThemeId>;
	const LABEL: &'static str = "themes";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.themes
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}
}
