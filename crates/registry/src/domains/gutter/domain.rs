pub struct Gutters;

impl crate::db::domain::DomainSpec for Gutters {
	type Input = super::GutterInput;
	type Entry = super::GutterEntry;
	type Id = crate::core::GutterId;
	type Runtime = crate::core::RuntimeRegistry<super::GutterEntry, crate::core::GutterId>;
	const LABEL: &'static str = "gutters";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.gutters
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}
}
