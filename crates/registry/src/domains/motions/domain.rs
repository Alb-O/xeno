pub struct Motions;

impl crate::db::domain::DomainSpec for Motions {
	type Input = super::MotionInput;
	type Entry = super::MotionEntry;
	type Id = crate::core::MotionId;
	type Runtime = crate::core::RuntimeRegistry<super::MotionEntry, crate::core::MotionId>;
	const LABEL: &'static str = "motions";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.motions
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}
}
