//! LSP server domain registration and runtime entry construction.

pub mod entry;
pub mod loader;
pub mod registry;
pub mod spec;

pub use entry::{LspServerEntry, LspServerInput};
pub use registry::LspServersRegistry;

/// Registers compiled LSP servers from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_lsp_servers_spec();
	let linked = entry::link_lsp_servers(&spec);

	for def in linked {
		db.push_domain::<LspServers>(LspServerInput::Linked(def));
	}
}

pub struct LspServers;

impl crate::db::domain::DomainSpec for LspServers {
	type Input = LspServerInput;
	type Entry = LspServerEntry;
	type Id = crate::core::symbol::LspServerId;
	const LABEL: &'static str = "lsp_servers";

	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.lsp_servers
	}
}
