//! Editor-side LSP integration root. See `xeno_lsp::session::manager` for full LSP architecture.

#[cfg(feature = "lsp")]
use xeno_lsp::LspManager;

use crate::buffer::Buffer;

#[cfg(feature = "lsp")]
pub struct LspSystem {
	pub(super) inner: RealLspSystem,
}

#[cfg(not(feature = "lsp"))]
pub struct LspSystem;

#[cfg(feature = "lsp")]
#[derive(Clone)]
pub struct LspHandle {
	sync: xeno_lsp::DocumentSync,
}

#[cfg(feature = "lsp")]
impl LspHandle {
	pub async fn close_document(
		&self,
		path: std::path::PathBuf,
		language: String,
	) -> xeno_lsp::Result<()> {
		self.sync.close_document(&path, &language).await
	}
}

#[cfg(feature = "lsp")]
pub(super) struct RealLspSystem {
	manager: LspManager,
	pub(super) sync_manager: crate::lsp::sync_manager::LspSyncManager,
	pub(super) completion: xeno_lsp::CompletionController,
	pub(super) signature_gen: u64,
	pub(super) signature_cancel: Option<tokio_util::sync::CancellationToken>,
	pub(super) ui_tx: tokio::sync::mpsc::UnboundedSender<crate::lsp::LspUiEvent>,
	pub(super) ui_rx: tokio::sync::mpsc::UnboundedReceiver<crate::lsp::LspUiEvent>,
}

#[cfg(feature = "lsp")]
impl LspSystem {
	pub fn new() -> Self {
		let (ui_tx, ui_rx) = tokio::sync::mpsc::unbounded_channel();

		let transport = xeno_lsp::LocalTransport::new();
		let manager = LspManager::new(transport);
		manager.spawn_router();

		Self {
			inner: RealLspSystem {
				manager,
				sync_manager: crate::lsp::sync_manager::LspSyncManager::new(),
				completion: xeno_lsp::CompletionController::new(),
				signature_gen: 0,
				signature_cancel: None,
				ui_tx,
				ui_rx,
			},
		}
	}

	pub fn handle(&self) -> LspHandle {
		LspHandle {
			sync: self.inner.manager.sync().clone(),
		}
	}
}

#[cfg(not(feature = "lsp"))]
impl LspSystem {
	pub fn new() -> Self {
		Self
	}
}

impl Default for LspSystem {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(feature = "lsp")]
impl LspSystem {
	pub fn poll_diagnostics(&mut self) -> Vec<xeno_lsp::DiagnosticsEvent> {
		self.inner.manager.poll_diagnostics()
	}

	pub fn diagnostics_version(&self) -> u64 {
		self.inner.manager.diagnostics_version()
	}

	pub fn configure_server(
		&self,
		language: impl Into<String>,
		config: xeno_lsp::LanguageServerConfig,
	) {
		self.inner.manager.configure_server(language, config);
	}

	pub fn remove_server(&self, language: &str) {
		self.inner.manager.remove_server(language);
	}

	pub fn sync(&self) -> &xeno_lsp::DocumentSync {
		self.inner.manager.sync()
	}

	pub fn sync_clone(&self) -> xeno_lsp::DocumentSync {
		self.inner.manager.sync().clone()
	}

	pub fn registry(&self) -> &xeno_lsp::Registry {
		self.inner.manager.registry()
	}

	pub fn documents(&self) -> &xeno_lsp::DocumentStateManager {
		self.inner.manager.documents()
	}

	pub fn get_diagnostics(&self, buffer: &Buffer) -> Vec<xeno_lsp::lsp_types::Diagnostic> {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync().get_diagnostics(p))
			.unwrap_or_default()
	}

	pub fn error_count(&self, buffer: &Buffer) -> usize {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync().error_count(p))
			.unwrap_or(0)
	}

	pub fn warning_count(&self, buffer: &Buffer) -> usize {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync().warning_count(p))
			.unwrap_or(0)
	}

	pub fn total_error_count(&self) -> usize {
		self.inner.manager.sync().total_error_count()
	}

	pub fn total_warning_count(&self) -> usize {
		self.inner.manager.sync().total_warning_count()
	}

	pub async fn shutdown_all(&self) {
		self.inner.manager.shutdown_all().await;
	}

	pub(crate) fn sync_manager(&self) -> &crate::lsp::sync_manager::LspSyncManager {
		&self.inner.sync_manager
	}

	pub(crate) fn sync_manager_mut(&mut self) -> &mut crate::lsp::sync_manager::LspSyncManager {
		&mut self.inner.sync_manager
	}
}
