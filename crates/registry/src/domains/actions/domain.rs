pub struct Actions;

impl crate::db::domain::DomainSpec for Actions {
	type Input = super::def::ActionInput;
	type Entry = super::entry::ActionEntry;
	type Id = crate::core::ActionId;
	type Runtime = crate::core::RuntimeRegistry<super::entry::ActionEntry, crate::core::ActionId>;
	const LABEL: &'static str = "actions";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.actions
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}

	fn on_push(_db: &mut crate::db::builder::RegistryDbBuilder, _input: &Self::Input) {}
}
