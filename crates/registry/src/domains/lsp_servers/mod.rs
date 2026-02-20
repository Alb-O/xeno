//! LSP server domain registration and runtime entry construction.

#[path = "compile/builtins.rs"]
pub mod builtins;
mod domain;
#[path = "contract/entry.rs"]
pub mod entry;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "runtime/query.rs"]
pub mod query;
#[path = "contract/spec.rs"]
pub mod spec;

pub use builtins::register_builtins;
pub use domain::LspServers;
pub use entry::{LspServerEntry, LspServerInput};
pub use query::LspServersRegistry;

/// Registers compiled LSP servers from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_lsp_servers_spec();
	let linked = entry::link_lsp_servers(&spec);

	for def in linked {
		db.push_domain::<LspServers>(LspServerInput::Linked(def));
	}
}
