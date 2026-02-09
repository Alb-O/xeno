pub mod entry;
pub mod loader;
pub mod registry;
pub mod spec;

pub use entry::{LspServerEntry, LspServerInput};
pub use registry::LspServersRegistry;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), crate::error::RegistryError> {
	register_compiled(db);
	Ok(())
}

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
	type StaticDef = entry::LspServerDef;
	type LinkedDef = crate::core::LinkedDef<entry::LspServerPayload>;
	const LABEL: &'static str = "lsp_servers";

	fn static_to_input(def: &'static Self::StaticDef) -> Self::Input {
		LspServerInput::Static(def.clone())
	}

	fn linked_to_input(def: Self::LinkedDef) -> Self::Input {
		LspServerInput::Linked(def)
	}

	fn builder(
		db: &mut crate::db::builder::RegistryDbBuilder,
	) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.lsp_servers
	}
}
