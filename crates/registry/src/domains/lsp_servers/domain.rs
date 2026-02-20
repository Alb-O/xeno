pub struct LspServers;

impl crate::db::domain::DomainSpec for LspServers {
	type Input = super::entry::LspServerInput;
	type Entry = super::entry::LspServerEntry;
	type Id = crate::core::symbol::LspServerId;
	type Runtime = super::query::LspServersRegistry;
	const LABEL: &'static str = "lsp_servers";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.lsp_servers
	}

	fn into_runtime(index: crate::core::index::RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime {
		crate::core::RuntimeRegistry::new(Self::LABEL, index)
	}
}
