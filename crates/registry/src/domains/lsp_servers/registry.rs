pub use crate::core::{DenseId, RegistryIndex, RegistryRef, RuntimeRegistry};
use crate::lsp_servers::LspServerEntry;
use crate::symbol::LspServerId;

pub type LspServerRef = RegistryRef<LspServerEntry, LspServerId>;

pub struct LspServersRegistry {
	pub(super) inner: RuntimeRegistry<LspServerEntry, LspServerId>,
}

impl LspServersRegistry {
	pub fn new(builtins: RegistryIndex<LspServerEntry, LspServerId>) -> Self {
		Self {
			inner: RuntimeRegistry::new("lsp_servers", builtins),
		}
	}

	pub fn get(&self, name: &str) -> Option<LspServerRef> {
		self.inner.get(name)
	}

	pub fn get_by_id(&self, id: LspServerId) -> Option<LspServerRef> {
		self.inner.get_by_id(id)
	}

	pub fn all(&self) -> Vec<LspServerRef> {
		self.inner.all()
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}
}
