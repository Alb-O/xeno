//! Editor-side LSP integration root. See `xeno_lsp::session::manager` for full LSP architecture.

#[cfg(feature = "lsp")]
use xeno_lsp::{LspRuntime, LspSession};
#[cfg(feature = "lsp")]
use xeno_primitives::{CommitResult, Rope, Transaction};
use xeno_worker::WorkerRuntime;

#[cfg(feature = "lsp")]
use crate::buffer::Buffer;

#[cfg(feature = "lsp")]
pub(crate) struct LspSystem {
	pub(super) inner: RealLspSystem,
}

#[cfg(not(feature = "lsp"))]
pub(crate) struct LspSystem;

#[cfg(feature = "lsp")]
#[derive(Clone)]
pub struct LspHandle {
	sync: xeno_lsp::DocumentSync,
}

#[cfg(feature = "lsp")]
impl LspHandle {
	pub async fn close_document(&self, path: std::path::PathBuf, language: String) -> xeno_lsp::Result<()> {
		self.sync.close_document(&path, &language).await
	}

	pub async fn on_buffer_close(&self, path: std::path::PathBuf, language: String) -> xeno_lsp::Result<()> {
		self.close_document(path, language).await
	}
}

#[cfg(feature = "lsp")]
pub(super) struct RealLspSystem {
	pub(super) session: LspSession,
	pub(super) runtime: LspRuntime,
	pub(super) sync_manager: crate::lsp::sync_manager::LspSyncManager,
	pub(super) completion: xeno_lsp::CompletionController,
	pub(super) signature_gen: u64,
	pub(super) signature_cancel: Option<tokio_util::sync::CancellationToken>,
	pub(super) ui_tx: tokio::sync::mpsc::UnboundedSender<crate::lsp::LspUiEvent>,
	pub(super) ui_rx: tokio::sync::mpsc::UnboundedReceiver<crate::lsp::LspUiEvent>,
}

#[cfg(feature = "lsp")]
impl LspSystem {
	pub fn new(worker_runtime: WorkerRuntime) -> Self {
		let (ui_tx, ui_rx) = tokio::sync::mpsc::unbounded_channel();

		let transport = xeno_lsp::LocalTransport::new(worker_runtime.clone());
		let (session, runtime) = LspSession::new(transport, worker_runtime.clone());
		if let Err(err) = runtime.start() {
			tracing::error!(error = ?err, "failed to start LSP runtime");
		}

		Self {
			inner: RealLspSystem {
				session,
				runtime,
				sync_manager: crate::lsp::sync_manager::LspSyncManager::new(worker_runtime.clone()),
				completion: xeno_lsp::CompletionController::new(worker_runtime),
				signature_gen: 0,
				signature_cancel: None,
				ui_tx,
				ui_rx,
			},
		}
	}

	pub fn handle(&self) -> LspHandle {
		LspHandle {
			sync: self.inner.session.sync().clone(),
		}
	}
}

#[cfg(not(feature = "lsp"))]
impl LspSystem {
	pub fn new(_worker_runtime: WorkerRuntime) -> Self {
		Self
	}
}

impl Default for LspSystem {
	fn default() -> Self {
		Self::new(WorkerRuntime::new())
	}
}

#[cfg(feature = "lsp")]
impl LspSystem {
	pub(crate) fn poll_diagnostics(&mut self) -> Vec<xeno_lsp::DiagnosticsEvent> {
		self.inner.session.poll_diagnostics()
	}

	pub fn diagnostics_version(&self) -> u64 {
		self.inner.session.diagnostics_version()
	}

	pub fn configure_server(&self, language: impl Into<String>, config: crate::lsp::api::LanguageServerConfig) {
		let internal_config = config.into_xeno_lsp();
		self.inner.session.configure_server(language, internal_config);
	}

	pub fn remove_server(&self, language: &str) {
		self.inner.session.remove_server(language);
	}

	pub(crate) fn sync(&self) -> &xeno_lsp::DocumentSync {
		self.inner.session.sync()
	}

	pub(crate) fn sync_clone(&self) -> xeno_lsp::DocumentSync {
		self.inner.session.sync().clone()
	}

	pub(crate) fn registry(&self) -> &xeno_lsp::Registry {
		self.inner.session.registry()
	}

	pub(crate) fn documents(&self) -> &xeno_lsp::DocumentStateManager {
		self.inner.session.documents()
	}

	pub(crate) fn get_raw_diagnostics(&self, buffer: &Buffer) -> Vec<xeno_lsp::lsp_types::Diagnostic> {
		buffer.path().as_ref().map(|p| self.sync().get_diagnostics(p)).unwrap_or_default()
	}

	pub fn get_diagnostics(&self, buffer: &Buffer) -> Vec<crate::lsp::api::Diagnostic> {
		use xeno_lsp::lsp_types::DiagnosticSeverity as LspSeverity;

		use crate::lsp::api::{Diagnostic, DiagnosticSeverity};

		self.get_raw_diagnostics(buffer)
			.into_iter()
			.map(|d| Diagnostic {
				range: (
					d.range.start.line as usize,
					d.range.start.character as usize,
					d.range.end.line as usize,
					d.range.end.character as usize,
				),
				severity: match d.severity {
					Some(LspSeverity::ERROR) => DiagnosticSeverity::Error,
					Some(LspSeverity::WARNING) => DiagnosticSeverity::Warning,
					Some(LspSeverity::INFORMATION) => DiagnosticSeverity::Info,
					Some(LspSeverity::HINT) | None => DiagnosticSeverity::Hint,
					_ => DiagnosticSeverity::Hint,
				},
				message: d.message,
				source: d.source,
				code: d.code.map(|c| match c {
					xeno_lsp::lsp_types::NumberOrString::Number(n) => n.to_string(),
					xeno_lsp::lsp_types::NumberOrString::String(s) => s,
				}),
			})
			.collect()
	}

	pub fn error_count(&self, buffer: &Buffer) -> usize {
		buffer.path().as_ref().map(|p| self.sync().error_count(p)).unwrap_or(0)
	}

	pub fn warning_count(&self, buffer: &Buffer) -> usize {
		buffer.path().as_ref().map(|p| self.sync().warning_count(p)).unwrap_or(0)
	}

	pub fn total_error_count(&self) -> usize {
		self.inner.session.sync().total_error_count()
	}

	pub fn total_warning_count(&self) -> usize {
		self.inner.session.sync().total_warning_count()
	}

	pub fn on_local_edit(&mut self, buffer: &Buffer, before: Option<Rope>, tx: &Transaction, result: &CommitResult) {
		if !result.applied {
			return;
		}

		let doc_id = buffer.document_id();
		let Some(encoding) = self.incremental_encoding_for_buffer(buffer) else {
			self.sync_manager_mut().escalate_full(doc_id);
			return;
		};

		let Some(before) = before else {
			self.sync_manager_mut().escalate_full(doc_id);
			return;
		};

		match xeno_lsp::compute_lsp_changes(&before, tx, encoding) {
			xeno_lsp::IncrementalResult::Incremental(changes) => {
				if changes.is_empty() {
					return;
				}
				let lsp_bytes: usize = changes.iter().map(|c| c.new_text.len()).sum();
				self.sync_manager_mut()
					.on_doc_edit(doc_id, result.version_before, result.version_after, changes, lsp_bytes);
			}
			xeno_lsp::IncrementalResult::FallbackToFull => {
				self.sync_manager_mut().escalate_full(doc_id);
			}
		}
	}

	pub async fn shutdown_all(&self) {
		self.inner.runtime.shutdown().await;
		self.inner.session.shutdown_all().await;
	}

	pub(crate) fn sync_manager(&self) -> &crate::lsp::sync_manager::LspSyncManager {
		&self.inner.sync_manager
	}

	pub(crate) fn sync_manager_mut(&mut self) -> &mut crate::lsp::sync_manager::LspSyncManager {
		&mut self.inner.sync_manager
	}

	pub(crate) fn completion_generation(&self) -> u64 {
		self.inner.completion.generation()
	}

	pub(crate) fn trigger_completion(&mut self, request: xeno_lsp::CompletionRequest<crate::buffer::ViewId>) {
		use crate::lsp::LspUiEvent;
		let ui_tx = self.inner.ui_tx.clone();
		self.inner.completion.trigger(request, move |generation, buffer_id, replace_start, response| {
			let _ = ui_tx.send(LspUiEvent::CompletionResult {
				generation,
				buffer_id,
				replace_start,
				response,
			});
		});
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

	pub(crate) fn set_signature_help_cancel(&mut self, cancel: tokio_util::sync::CancellationToken) {
		self.inner.signature_cancel = Some(cancel);
	}

	pub(crate) fn take_signature_help_cancel(&mut self) -> Option<tokio_util::sync::CancellationToken> {
		self.inner.signature_cancel.take()
	}

	pub(crate) fn ui_tx(&self) -> tokio::sync::mpsc::UnboundedSender<crate::lsp::LspUiEvent> {
		self.inner.ui_tx.clone()
	}

	pub(crate) fn try_recv_ui_event(&mut self) -> Option<crate::lsp::LspUiEvent> {
		self.inner.ui_rx.try_recv().ok()
	}

	pub(crate) fn canonicalize_path(&self, path: &std::path::Path) -> std::path::PathBuf {
		path.canonicalize().unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(path))
	}
}
