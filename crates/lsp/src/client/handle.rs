//! Public handle to an LSP language server.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use lsp_types::{ServerCapabilities, Uri};
use tokio::sync::{Notify, OnceCell};

use super::config::{LanguageServerId, OffsetEncoding};
use super::transport::LspTransport;
use crate::{AnyNotification, AnyRequest, RequestId, Result};

/// Handle to an LSP language server.
///
/// This provides a high-level API for communicating with a language server.
/// It uses an underlying [`LspTransport`] for actual communication.
#[derive(Clone)]
pub struct ClientHandle {
	/// Unique identifier for this client.
	pub(super) id: LanguageServerId,
	/// Human-readable name (usually the command name).
	pub(super) name: String,
	/// Server capabilities (set after initialization).
	pub(super) capabilities: Arc<OnceCell<ServerCapabilities>>,
	/// Root path for the workspace.
	pub(super) root_path: PathBuf,
	/// Root URI for the workspace.
	pub(super) root_uri: Option<Uri>,
	/// Notification channel for initialization completion.
	pub(super) initialize_notify: Arc<Notify>,
	/// Per-request timeout.
	pub(super) timeout: Duration,
	/// Underlying transport.
	pub(super) transport: Arc<dyn LspTransport>,
	/// Whether the server has completed initialization.
	pub(super) is_ready: Arc<AtomicBool>,
	/// Monotonic request ID generator for this client (shared across clones).
	pub(super) next_request_id: Arc<AtomicU64>,
}

impl std::fmt::Debug for ClientHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ClientHandle")
			.field("id", &self.id)
			.field("name", &self.name)
			.field("root_path", &self.root_path)
			.field("initialized", &self.capabilities.initialized())
			.field("ready", &self.is_ready.load(Ordering::Relaxed))
			.finish_non_exhaustive()
	}
}

impl ClientHandle {
	/// Create a new client handle.
	pub fn new(id: LanguageServerId, name: String, root_path: PathBuf, transport: Arc<dyn LspTransport>) -> Self {
		let root_uri = crate::uri_from_path(&root_path);
		Self {
			id,
			name,
			capabilities: Arc::new(OnceCell::new()),
			root_path,
			root_uri,
			initialize_notify: Arc::new(Notify::new()),
			timeout: Duration::from_secs(30),
			transport,
			is_ready: Arc::new(AtomicBool::new(false)),
			next_request_id: Arc::new(AtomicU64::new(1)),
		}
	}

	/// Get the client's unique identifier.
	pub fn id(&self) -> LanguageServerId {
		self.id
	}

	/// Get the client's name.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Check if the server has been initialized.
	pub fn is_initialized(&self) -> bool {
		self.capabilities.initialized()
	}

	/// Check if the server is ready for requests.
	///
	/// Uses `Acquire` ordering so that all writes performed before the
	/// corresponding `set_ready(true)` (including `capabilities.set()`)
	/// are visible to the caller.
	pub fn is_ready(&self) -> bool {
		self.is_ready.load(Ordering::Acquire)
	}

	/// Set the server's ready state.
	///
	/// Uses `Release` ordering to ensure all prior writes (capabilities,
	/// notify, etc.) are visible to any thread that observes `is_ready() == true`.
	pub(crate) fn set_ready(&self, ready: bool) {
		debug_assert!(!ready || self.is_initialized(), "set_ready(true) called before capabilities were set");
		self.is_ready.store(ready, Ordering::Release);
	}

	/// Get the server's capabilities if initialized.
	///
	/// Returns `None` before the initialize handshake completes.
	pub fn capabilities(&self) -> Option<&ServerCapabilities> {
		self.capabilities.get()
	}

	/// Check if the server supports hover.
	pub fn supports_hover(&self) -> bool {
		self.capabilities().is_some_and(|c| c.hover_provider.is_some())
	}

	/// Check if the server supports completion.
	pub fn supports_completion(&self) -> bool {
		self.capabilities().is_some_and(|c| c.completion_provider.is_some())
	}

	/// Check if the server supports formatting.
	pub fn supports_formatting(&self) -> bool {
		self.capabilities().is_some_and(|c| c.document_formatting_provider.is_some())
	}

	/// Check if the server supports go to definition.
	pub fn supports_definition(&self) -> bool {
		self.capabilities().is_some_and(|c| c.definition_provider.is_some())
	}

	/// Check if the server supports find references.
	pub fn supports_references(&self) -> bool {
		self.capabilities().is_some_and(|c| c.references_provider.is_some())
	}

	/// Check if the server supports document symbols.
	pub fn supports_document_symbol(&self) -> bool {
		self.capabilities().is_some_and(|c| c.document_symbol_provider.is_some())
	}

	/// Check if the server supports code actions.
	pub fn supports_code_action(&self) -> bool {
		self.capabilities().is_some_and(|c| c.code_action_provider.is_some())
	}

	/// Check if the server supports signature help.
	pub fn supports_signature_help(&self) -> bool {
		self.capabilities().is_some_and(|c| c.signature_help_provider.is_some())
	}

	/// Check if the server supports rename.
	///
	/// Returns `false` when the server explicitly sets `renameProvider: false`
	/// or omits it entirely.
	pub fn supports_rename(&self) -> bool {
		self.capabilities()
			.is_some_and(|c| matches!(c.rename_provider, Some(lsp_types::OneOf::Left(true)) | Some(lsp_types::OneOf::Right(_))))
	}

	/// Check if the server supports prepareRename (rename validation before execution).
	pub fn supports_prepare_rename(&self) -> bool {
		self.capabilities().is_some_and(|c| {
			matches!(
				c.rename_provider,
				Some(lsp_types::OneOf::Right(lsp_types::RenameOptions {
					prepare_provider: Some(true),
					..
				}))
			)
		})
	}

	/// Check if the server supports execute command.
	pub fn supports_execute_command(&self) -> bool {
		self.capabilities().is_some_and(|c| c.execute_command_provider.is_some())
	}

	/// Check if the server supports go to declaration.
	pub fn supports_declaration(&self) -> bool {
		self.capabilities().is_some_and(|c| c.declaration_provider.is_some())
	}

	/// Check if the server supports go to implementation.
	pub fn supports_implementation(&self) -> bool {
		self.capabilities().is_some_and(|c| c.implementation_provider.is_some())
	}

	/// Check if the server supports go to type definition.
	pub fn supports_type_definition(&self) -> bool {
		self.capabilities().is_some_and(|c| c.type_definition_provider.is_some())
	}

	/// Check if the server supports range formatting.
	pub fn supports_range_formatting(&self) -> bool {
		self.capabilities().is_some_and(|c| c.document_range_formatting_provider.is_some())
	}

	/// Check if the server supports workspace symbol search.
	pub fn supports_workspace_symbol(&self) -> bool {
		self.capabilities().is_some_and(|c| c.workspace_symbol_provider.is_some())
	}

	/// Check if the server is interested in willRenameFiles requests.
	pub fn supports_will_rename_files(&self) -> bool {
		self.capabilities().is_some_and(|c| {
			c.workspace
				.as_ref()
				.and_then(|w| w.file_operations.as_ref())
				.and_then(|fo| fo.will_rename.as_ref())
				.is_some()
		})
	}

	/// Check if the server is interested in didRenameFiles notifications.
	pub fn supports_did_rename_files(&self) -> bool {
		self.capabilities().is_some_and(|c| {
			c.workspace
				.as_ref()
				.and_then(|w| w.file_operations.as_ref())
				.and_then(|fo| fo.did_rename.as_ref())
				.is_some()
		})
	}

	/// Check if the server is interested in willCreateFiles requests.
	pub fn supports_will_create_files(&self) -> bool {
		self.capabilities().is_some_and(|c| {
			c.workspace
				.as_ref()
				.and_then(|w| w.file_operations.as_ref())
				.and_then(|fo| fo.will_create.as_ref())
				.is_some()
		})
	}

	/// Check if the server is interested in didCreateFiles notifications.
	pub fn supports_did_create_files(&self) -> bool {
		self.capabilities().is_some_and(|c| {
			c.workspace
				.as_ref()
				.and_then(|w| w.file_operations.as_ref())
				.and_then(|fo| fo.did_create.as_ref())
				.is_some()
		})
	}

	/// Check if the server is interested in willDeleteFiles requests.
	pub fn supports_will_delete_files(&self) -> bool {
		self.capabilities().is_some_and(|c| {
			c.workspace
				.as_ref()
				.and_then(|w| w.file_operations.as_ref())
				.and_then(|fo| fo.will_delete.as_ref())
				.is_some()
		})
	}

	/// Check if the server is interested in didDeleteFiles notifications.
	pub fn supports_did_delete_files(&self) -> bool {
		self.capabilities().is_some_and(|c| {
			c.workspace
				.as_ref()
				.and_then(|w| w.file_operations.as_ref())
				.and_then(|fo| fo.did_delete.as_ref())
				.is_some()
		})
	}

	/// Get the offset encoding negotiated with the server.
	///
	/// Returns the LSP default (UTF-16) if the server has not yet finished
	/// initialization and capabilities are unavailable.
	pub fn offset_encoding(&self) -> OffsetEncoding {
		self.capabilities()
			.and_then(|c| c.position_encoding.as_ref())
			.and_then(OffsetEncoding::from_lsp)
			.unwrap_or_default()
	}

	/// Get the root path.
	pub fn root_path(&self) -> &Path {
		&self.root_path
	}

	/// Get the root URI.
	pub fn root_uri(&self) -> Option<&Uri> {
		self.root_uri.as_ref()
	}

	/// Wait for initialization to complete.
	pub async fn wait_initialized(&self) {
		if self.is_initialized() {
			return;
		}
		self.initialize_notify.notified().await;
	}

	/// Send a request to the language server.
	///
	/// A unique monotonic request ID is automatically generated and assigned to the outgoing
	/// request. This ID is used by the underlying transport to correlate the response.
	///
	/// # Errors
	/// Returns an error if the transport fails to send the request, if the request times out,
	/// or if the server returns an LSP error response.
	pub async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result> {
		let id_num = self.next_request_id.fetch_add(1, Ordering::Relaxed);
		let req = AnyRequest::new(
			RequestId::Number(id_num as i32),
			R::METHOD,
			serde_json::to_value(params).expect("Failed to serialize"),
		);
		let resp = self.transport.request(self.id, req, Some(self.timeout)).await?;
		match resp.error {
			None => Ok(serde_json::from_value(resp.result.unwrap_or_default())?),
			Some(err) => Err(crate::Error::Response(err)),
		}
	}

	/// Send a notification to the language server.
	pub async fn notify<N: Notification>(&self, params: N::Params) -> Result<()> {
		let notif = AnyNotification::new(N::METHOD, serde_json::to_value(params).expect("Failed to serialize"));
		self.transport.notify(self.id, notif).await
	}
}
