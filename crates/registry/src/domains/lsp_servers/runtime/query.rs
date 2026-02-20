//! LSP server runtime query surface.

pub use crate::core::{DenseId, RegistryIndex, RegistryRef, RuntimeRegistry};
use crate::lsp_servers::LspServerEntry;
use crate::symbol::LspServerId;

pub type LspServerRef = RegistryRef<LspServerEntry, LspServerId>;
pub type LspServersRegistry = RuntimeRegistry<LspServerEntry, LspServerId>;
