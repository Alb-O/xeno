use crate::core::{RegistryMeta, Symbol};

/// Symbolized snippet entry.
#[derive(Clone)]
pub struct SnippetEntry {
	pub meta: RegistryMeta,
	pub body: Symbol,
}

crate::impl_registry_entry!(SnippetEntry);
