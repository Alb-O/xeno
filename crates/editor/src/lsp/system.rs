#[cfg(feature = "lsp")]
use std::path::Path;

#[cfg(feature = "lsp")]
use xeno_lsp::lsp_types::{TextDocumentSyncCapability, TextDocumentSyncKind};
#[cfg(feature = "lsp")]
use xeno_lsp::{LspManager, OffsetEncoding};

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
	manager: LspManager,
	sync_manager: crate::lsp::sync_manager::LspSyncManager,
	completion: xeno_lsp::CompletionController,
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
				manager: LspManager::new(),
				sync_manager: crate::lsp::sync_manager::LspSyncManager::new(),
				completion: xeno_lsp::CompletionController::new(),
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

	pub fn sync_clone(&self) -> xeno_lsp::DocumentSync {
		self.inner.manager.sync().clone()
	}

	pub fn registry(&self) -> &xeno_lsp::Registry {
		self.inner.manager.registry()
	}

	pub fn documents(&self) -> &xeno_lsp::DocumentStateManager {
		self.inner.manager.documents()
	}

	fn canonicalize_path(&self, path: &std::path::Path) -> std::path::PathBuf {
		path.canonicalize()
			.unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(path))
	}

	pub async fn on_buffer_open(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::ClientHandle>> {
		let Some(path) = buffer.path() else {
			return Ok(None);
		};
		let Some(language) = &buffer.file_type() else {
			return Ok(None);
		};

		if self.registry().get_config(language).is_none() {
			return Ok(None);
		}

		let abs_path = self.canonicalize_path(&path);

		let content = buffer.with_doc(|doc| doc.content().clone());
		let client = self
			.sync()
			.open_document(&abs_path, language, &content)
			.await?;
		Ok(Some(client))
	}

	pub fn on_buffer_will_save(&self, buffer: &Buffer) -> xeno_lsp::Result<()> {
		let Some(path) = buffer.path() else {
			return Ok(());
		};
		let Some(language) = buffer.file_type() else {
			return Ok(());
		};
		let abs_path = self.canonicalize_path(&path);
		self.sync().notify_will_save(&abs_path, &language)
	}

	pub fn on_buffer_did_save(&self, buffer: &Buffer, include_text: bool) -> xeno_lsp::Result<()> {
		let Some(path) = buffer.path() else {
			return Ok(());
		};
		let Some(language) = buffer.file_type() else {
			return Ok(());
		};
		let abs_path = self.canonicalize_path(&path);
		let text = buffer.with_doc(|doc| {
			if include_text {
				Some(doc.content().clone())
			} else {
				None
			}
		});
		self.sync()
			.notify_did_save(&abs_path, &language, include_text, text.as_ref())
	}

	pub fn on_buffer_close(&self, buffer: &Buffer) -> xeno_lsp::Result<()> {
		let Some(path) = buffer.path() else {
			return Ok(());
		};
		let Some(language) = buffer.file_type() else {
			return Ok(());
		};
		let abs_path = self.canonicalize_path(&path);
		self.sync().close_document(&abs_path, &language)
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
		let Some(path) = buffer.path() else {
			return Ok(None);
		};
		let Some(language) = buffer.file_type() else {
			return Ok(None);
		};

		let abs_path = self.canonicalize_path(&path);

		let Some(client) = self.sync().registry().get(&language, &abs_path) else {
			return Ok(None);
		};

		let uri = xeno_lsp::uri_from_path(&abs_path)
			.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid path".into()))?;

		let encoding = client.offset_encoding();
		let position = buffer
			.with_doc(|doc| xeno_lsp::char_to_lsp_position(doc.content(), buffer.cursor, encoding))
			.ok_or_else(|| xeno_lsp::Error::Protocol("Invalid position".into()))?;

		Ok(Some((client, uri, position)))
	}

	pub async fn hover(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::Hover>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.hover(uri, position).await
	}

	pub async fn completion(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::CompletionResponse>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.completion(uri, position, None).await
	}

	pub async fn goto_definition(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<xeno_lsp::lsp_types::GotoDefinitionResponse>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.goto_definition(uri, position).await
	}

	pub async fn references(
		&self,
		buffer: &Buffer,
		include_declaration: bool,
	) -> xeno_lsp::Result<Option<Vec<xeno_lsp::lsp_types::Location>>> {
		let Some((client, uri, position)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		client.references(uri, position, include_declaration).await
	}

	pub async fn format(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<Vec<xeno_lsp::lsp_types::TextEdit>>> {
		let Some((client, uri, _)) = self.prepare_position_request(buffer)? else {
			return Ok(None);
		};
		let options = xeno_lsp::lsp_types::FormattingOptions {
			tab_size: 4,
			insert_spaces: false,
			..Default::default()
		};
		client.formatting(uri, options).await
	}

	pub async fn shutdown_all(&self) {
		self.inner.manager.shutdown_all().await;
	}

	pub fn incremental_encoding_for_buffer(
		&self,
		buffer: &Buffer,
	) -> Option<xeno_lsp::OffsetEncoding> {
		let path = buffer.path()?;
		let language = buffer.file_type()?;
		self.incremental_encoding(&path, &language)
	}

	pub fn offset_encoding_for_buffer(&self, buffer: &Buffer) -> xeno_lsp::OffsetEncoding {
		let Some(path) = buffer.path() else {
			return OffsetEncoding::Utf16;
		};
		let Some(language) = buffer.file_type() else {
			return OffsetEncoding::Utf16;
		};

		self.sync()
			.registry()
			.get(&language, &path)
			.map(|client| client.offset_encoding())
			.unwrap_or(OffsetEncoding::Utf16)
	}

	fn incremental_encoding(&self, path: &Path, language: &str) -> Option<OffsetEncoding> {
		let client = self.sync().registry().get(language, path)?;
		let caps = client.try_capabilities()?;
		let supports_incremental = match &caps.text_document_sync {
			Some(TextDocumentSyncCapability::Kind(kind)) => {
				*kind == TextDocumentSyncKind::INCREMENTAL
			}
			Some(TextDocumentSyncCapability::Options(options)) => {
				matches!(options.change, Some(TextDocumentSyncKind::INCREMENTAL))
			}
			None => false,
		};

		if supports_incremental {
			Some(client.offset_encoding())
		} else {
			None
		}
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
		request: xeno_lsp::CompletionRequest<crate::buffer::ViewId>,
	) {
		use crate::lsp::LspUiEvent;
		let ui_tx = self.inner.ui_tx.clone();
		self.inner.completion.trigger(
			request,
			move |generation, buffer_id, replace_start, response| {
				let _ = ui_tx.send(LspUiEvent::CompletionResult {
					generation,
					buffer_id,
					replace_start,
					response,
				});
			},
		);
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
			.overlays()
			.get::<CompletionState>()
			.cloned()
			.unwrap_or_default();
		if !completions.active || completions.items.is_empty() {
			return;
		}

		let Some(menu_state) = editor
			.overlays()
			.get::<LspMenuState>()
			.and_then(|s: &LspMenuState| s.active())
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
	pub fn render_completion_popup(
		&self,
		_editor: &crate::impls::Editor,
		_frame: &mut xeno_tui::Frame,
	) {
		// No-op when LSP is disabled
	}
}
