pub struct Hooks;

impl crate::db::domain::DomainSpec for Hooks {
	type Input = super::HookInput;
	type Entry = super::HookEntry;
	type Id = crate::core::HookId;
	type Runtime = super::HooksRegistry;
	const LABEL: &'static str = "hooks";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.hooks
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		super::HooksRegistry::new(index)
	}

	fn on_push(_db: &mut crate::db::builder::RegistryDbBuilder, _input: &Self::Input) {}
}
