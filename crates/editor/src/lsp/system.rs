use crate::buffer::Buffer;
use crate::render::{DiagnosticLineMap, DiagnosticRangeMap};

#[cfg(feature = "lsp")]
pub struct LspSystem {
	inner: RealLspSystem,
}

#[cfg(not(feature = "lsp"))]
pub struct LspSystem;

#[cfg(feature = "lsp")]
struct RealLspSystem {
	manager: crate::lsp::LspManager,
	sync_manager: crate::lsp::sync_manager::LspSyncManager,
	completion: crate::lsp::CompletionController,
	signature_gen: u64,
	signature_cancel: Option<tokio_util::sync::CancellationToken>,
	ui_tx: tokio::sync::mpsc::UnboundedSender<crate::lsp::LspUiEvent>,
	ui_rx: tokio::sync::mpsc::UnboundedReceiver<crate::lsp::LspUiEvent>,
}

#[cfg(feature = "lsp")]
impl LspSystem {
	pub fn new() -> Self {
		let (ui_tx, ui_rx) = tokio::sync::mpsc::unbounded_channel();
		Self {
			inner: RealLspSystem {
				manager: crate::lsp::LspManager::new(),
				sync_manager: crate::lsp::sync_manager::LspSyncManager::new(),
				completion: crate::lsp::CompletionController::new(),
				signature_gen: 0,
				signature_cancel: None,
				ui_tx,
				ui_rx,
			},
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

	/// Get a cloned sync handle for background LSP operations.
	///
	/// This allows spawning background tasks that can open documents
	/// without blocking the main thread.
	pub fn sync_clone(&self) -> xeno_lsp::DocumentSync {
		self.inner.manager.sync().clone()
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

	pub(crate) fn sync_manager(&self) -> &crate::lsp::sync_manager::LspSyncManager {
		&self.inner.sync_manager
	}

	pub(crate) fn sync_manager_mut(&mut self) -> &mut crate::lsp::sync_manager::LspSyncManager {
		&mut self.inner.sync_manager
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

	pub fn get_diagnostic_line_map(&self, buffer: &Buffer) -> DiagnosticLineMap {
		use crate::lsp::diagnostics::build_diagnostic_line_map;
		let diagnostics = self.get_diagnostics(buffer);
		build_diagnostic_line_map(&diagnostics)
	}

	pub fn get_diagnostic_range_map(&self, buffer: &Buffer) -> DiagnosticRangeMap {
		use crate::lsp::diagnostics::build_diagnostic_range_map;
		let diagnostics = self.get_diagnostics(buffer);
		build_diagnostic_range_map(&diagnostics)
	}

	/// Renders the LSP completion popup if active.
	pub fn render_completion_popup(
		&self,
		editor: &crate::impls::Editor,
		frame: &mut xeno_tui::Frame,
	) {
		use xeno_tui::layout::Rect;

		use crate::completion::CompletionState;
		use crate::lsp::{LspMenuKind, LspMenuState};

		let completions = editor
			.state
			.overlays
			.get::<CompletionState>()
			.cloned()
			.unwrap_or_default();
		if !completions.active || completions.items.is_empty() {
			return;
		}

		let Some(menu_state) = editor
			.state
			.overlays
			.get::<LspMenuState>()
			.and_then(|s| s.active())
		else {
			return;
		};
		let buffer_id = match menu_state {
			LspMenuKind::Completion { buffer_id, .. } => *buffer_id,
			LspMenuKind::CodeAction { buffer_id, .. } => *buffer_id,
		};
		if buffer_id != editor.focused_view() {
			return;
		}

		let Some(buffer) = editor.get_buffer(buffer_id) else {
			return;
		};
		let tab_width = editor.tab_width_for(buffer_id);
		let Some((cursor_row, cursor_col)) =
			buffer.doc_to_screen_position(buffer.cursor, tab_width)
		else {
			return;
		};

		let max_label_len = completions
			.items
			.iter()
			.map(|it| it.label.len())
			.max()
			.unwrap_or(0);
		let width = (max_label_len + 10).max(12);
		let height = completions
			.items
			.len()
			.clamp(1, CompletionState::MAX_VISIBLE);

		let view_area = editor.focused_view_area();
		let mut x = view_area.x.saturating_add(cursor_col);
		let mut y = view_area.y.saturating_add(cursor_row.saturating_add(1));

		let width_u16 = width.min(view_area.width as usize) as u16;
		let height_u16 = height.min(view_area.height as usize) as u16;

		if x + width_u16 > view_area.right() {
			x = view_area.right().saturating_sub(width_u16);
		}
		if y + height_u16 > view_area.bottom() {
			let above = view_area
				.y
				.saturating_add(cursor_row)
				.saturating_sub(height_u16);
			y = above.max(view_area.y);
		}

		let area = Rect::new(x, y, width_u16, height_u16);
		frame.render_widget(editor.render_completion_menu(area), area);
	}
}

#[cfg(not(feature = "lsp"))]
impl LspSystem {
	pub fn get_diagnostic_line_map(&self, _buffer: &Buffer) -> DiagnosticLineMap {
		DiagnosticLineMap::new()
	}

	pub fn get_diagnostic_range_map(&self, _buffer: &Buffer) -> DiagnosticRangeMap {
		DiagnosticRangeMap::new()
	}

	/// Renders the LSP completion popup if active.
	///
	/// No-op when LSP feature is disabled.
	pub fn render_completion_popup(
		&self,
		_editor: &crate::impls::Editor,
		_frame: &mut xeno_tui::Frame,
	) {
		// No-op when LSP is disabled
	}
}
