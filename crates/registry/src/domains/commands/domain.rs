pub struct Commands;

impl crate::db::domain::DomainSpec for Commands {
	type Input = super::def::CommandInput;
	type Entry = super::entry::CommandEntry;
	type Id = crate::core::CommandId;
	type Runtime = crate::core::RuntimeRegistry<super::entry::CommandEntry, crate::core::CommandId>;
	const LABEL: &'static str = "commands";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.commands
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}
}
