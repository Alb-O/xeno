pub struct Options;

impl crate::db::domain::DomainSpec for Options {
	type Input = super::OptionInput;
	type Entry = super::OptionEntry;
	type Id = crate::core::OptionId;
	type Runtime = super::OptionsRegistry;
	const LABEL: &'static str = "options";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.options
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}

	fn on_push(_db: &mut crate::db::builder::RegistryDbBuilder, input: &Self::Input) {
		if let super::OptionInput::Static(def) = input {
			crate::db::builder::validate_option_def(def);
		}
	}
}
