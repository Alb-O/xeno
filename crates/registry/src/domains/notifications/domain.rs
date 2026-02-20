pub struct Notifications;

impl crate::db::domain::DomainSpec for Notifications {
	type Input = super::NotificationInput;
	type Entry = super::NotificationEntry;
	type Id = crate::core::NotificationId;
	type Runtime = crate::core::RuntimeRegistry<super::NotificationEntry, crate::core::NotificationId>;
	const LABEL: &'static str = "notifications";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.notifications
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}
}
