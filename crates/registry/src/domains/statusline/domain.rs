pub struct Statusline;

impl crate::db::domain::DomainSpec for Statusline {
	type Input = super::StatuslineInput;
	type Entry = super::StatuslineEntry;
	type Id = crate::core::StatuslineId;
	type Runtime = crate::core::RuntimeRegistry<super::StatuslineEntry, crate::core::StatuslineId>;
	const LABEL: &'static str = "statusline";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.statusline
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}
}
