#[cfg(feature = "lsp")]
use crate::buffer::Buffer;

pub struct LspSystem {
	#[cfg(feature = "lsp")]
	inner: RealLspSystem,
	#[cfg(not(feature = "lsp"))]
	inner: NoopLspSystem,
}

#[cfg(feature = "lsp")]
struct RealLspSystem {
	manager: crate::lsp::LspManager,
	pending: crate::lsp::pending::PendingLspState,
	completion: crate::lsp::CompletionController,
	signature_gen: u64,
	signature_cancel: Option<tokio_util::sync::CancellationToken>,
	ui_tx: tokio::sync::mpsc::UnboundedSender<crate::lsp::LspUiEvent>,
	ui_rx: tokio::sync::mpsc::UnboundedReceiver<crate::lsp::LspUiEvent>,
}

#[cfg(not(feature = "lsp"))]
struct NoopLspSystem;

impl LspSystem {
	pub fn new() -> Self {
		#[cfg(feature = "lsp")]
		{
			let (ui_tx, ui_rx) = tokio::sync::mpsc::unbounded_channel();
			Self {
				inner: RealLspSystem {
					manager: crate::lsp::LspManager::new(),
					pending: crate::lsp::pending::PendingLspState::new(),
					completion: crate::lsp::CompletionController::new(),
					signature_gen: 0,
					signature_cancel: None,
					ui_tx,
					ui_rx,
				},
			}
		}

		#[cfg(not(feature = "lsp"))]
		{
			Self {
				inner: NoopLspSystem,
			}
		}
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

	pub fn registry(&self) -> &xeno_lsp::Registry {
		self.inner.manager.registry()
	}

	pub fn documents(&self) -> &xeno_lsp::DocumentStateManager {
		self.inner.manager.documents()
	}

	pub async fn on_buffer_open(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::ClientHandle>> {
		self.inner.manager.on_buffer_open(buffer).await
	}

	pub fn on_buffer_will_save(&self, buffer: &Buffer) -> xeno_lsp::Result<()> {
		self.inner.manager.on_buffer_will_save(buffer)
	}

	pub fn on_buffer_did_save(&self, buffer: &Buffer, include_text: bool) -> xeno_lsp::Result<()> {
		self.inner.manager.on_buffer_did_save(buffer, include_text)
	}

	pub fn on_buffer_close(&self, buffer: &Buffer) -> xeno_lsp::Result<()> {
		self.inner.manager.on_buffer_close(buffer)
	}

	pub fn get_diagnostics(&self, buffer: &Buffer) -> Vec<xeno_lsp::lsp_types::Diagnostic> {
		self.inner.manager.get_diagnostics(buffer)
	}

	pub fn error_count(&self, buffer: &Buffer) -> usize {
		self.inner.manager.error_count(buffer)
	}

	pub fn warning_count(&self, buffer: &Buffer) -> usize {
		self.inner.manager.warning_count(buffer)
	}

	pub fn total_error_count(&self) -> usize {
		self.inner.manager.total_error_count()
	}

	pub fn total_warning_count(&self) -> usize {
		self.inner.manager.total_warning_count()
	}

	pub(crate) fn prepare_position_request(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<
		Option<(
			xeno_lsp::ClientHandle,
			xeno_lsp::lsp_types::Uri,
			xeno_lsp::lsp_types::Position,
		)>,
	> {
		self.inner.manager.prepare_position_request(buffer)
	}

	pub async fn hover(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::Hover>> {
		self.inner.manager.hover(buffer).await
	}

	pub async fn completion(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::CompletionResponse>> {
		self.inner.manager.completion(buffer).await
	}

	pub async fn goto_definition(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::GotoDefinitionResponse>> {
		self.inner.manager.goto_definition(buffer).await
	}

	pub async fn references(
		&self,
		buffer: &Buffer,
		include_declaration: bool,
	) -> xeno_lsp::Result<Option<Vec<xeno_lsp::lsp_types::Location>>> {
		self.inner
			.manager
			.references(buffer, include_declaration)
			.await
	}

	pub async fn format(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<Vec<xeno_lsp::lsp_types::TextEdit>>> {
		self.inner.manager.format(buffer).await
	}

	pub async fn shutdown_all(&self) {
		self.inner.manager.shutdown_all().await;
	}

	pub fn incremental_encoding_for_buffer(
		&self,
		buffer: &Buffer,
	) -> Option<xeno_lsp::OffsetEncoding> {
		self.inner.manager.incremental_encoding_for_buffer(buffer)
	}

	pub fn offset_encoding_for_buffer(&self, buffer: &Buffer) -> xeno_lsp::OffsetEncoding {
		self.inner.manager.offset_encoding_for_buffer(buffer)
	}

	pub(crate) fn pending(&self) -> &crate::lsp::pending::PendingLspState {
		&self.inner.pending
	}

	pub(crate) fn pending_mut(&mut self) -> &mut crate::lsp::pending::PendingLspState {
		&mut self.inner.pending
	}

	pub(crate) fn completion_generation(&self) -> u64 {
		self.inner.completion.generation()
	}

	pub(crate) fn trigger_completion(
		&mut self,
		request: crate::lsp::completion_controller::CompletionRequest,
	) {
		self.inner.completion.trigger(request);
	}

	pub(crate) fn cancel_completion(&mut self) {
		self.inner.completion.cancel();
	}

	pub(crate) fn signature_help_generation(&self) -> u64 {
		self.inner.signature_gen
	}

	pub(crate) fn bump_signature_help_generation(&mut self) -> u64 {
		self.inner.signature_gen = self.inner.signature_gen.wrapping_add(1);
		self.inner.signature_gen
	}

	pub(crate) fn set_signature_help_cancel(
		&mut self,
		cancel: tokio_util::sync::CancellationToken,
	) {
		self.inner.signature_cancel = Some(cancel);
	}

	pub(crate) fn take_signature_help_cancel(
		&mut self,
	) -> Option<tokio_util::sync::CancellationToken> {
		self.inner.signature_cancel.take()
	}

	pub(crate) fn ui_tx(&self) -> tokio::sync::mpsc::UnboundedSender<crate::lsp::LspUiEvent> {
		self.inner.ui_tx.clone()
	}

	pub(crate) fn try_recv_ui_event(&mut self) -> Option<crate::lsp::LspUiEvent> {
		self.inner.ui_rx.try_recv().ok()
	}
}
