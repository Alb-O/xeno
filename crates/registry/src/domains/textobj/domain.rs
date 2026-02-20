pub struct TextObjects;

impl crate::db::domain::DomainSpec for TextObjects {
	type Input = super::TextObjectInput;
	type Entry = super::TextObjectEntry;
	type Id = crate::core::TextObjectId;
	type Runtime = super::TextObjectRegistry;
	const LABEL: &'static str = "text_objects";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.text_objects
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		super::TextObjectRegistry::new(index)
	}
}
