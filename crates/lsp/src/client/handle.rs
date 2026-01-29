//! Public handle to an LSP language server.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use lsp_types::{ServerCapabilities, Uri};
use tokio::sync::{Notify, OnceCell, mpsc, oneshot, watch};

use super::config::{LanguageServerId, OffsetEncoding};
use super::outbox::OutboundMsg;
use super::state::ServerState;
use crate::types::{AnyNotification, AnyRequest, RequestId};
use crate::{Error, Result};

/// Handle to an LSP language server.
///
/// This provides a high-level API for communicating with a language server.
/// The actual I/O and main loop run in a separate task - this handle uses
/// a message queue for outbound communication.
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
	/// Outbound message queue for serialized writes.
	pub(super) outbound_tx: mpsc::Sender<OutboundMsg>,
	/// Per-request timeout.
	pub(super) timeout: Duration,
	/// Server state broadcast channel (sender).
	pub(super) state_tx: watch::Sender<ServerState>,
}

impl std::fmt::Debug for ClientHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ClientHandle")
			.field("id", &self.id)
			.field("name", &self.name)
			.field("root_path", &self.root_path)
			.field("initialized", &self.capabilities.initialized())
			.finish_non_exhaustive()
	}
}

impl ClientHandle {
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

	/// Get the current server state.
	pub fn state(&self) -> ServerState {
		*self.state_tx.borrow()
	}

	/// Set the server state.
	pub fn set_state(&self, state: ServerState) {
		let _ = self.state_tx.send(state);
	}

	/// Subscribe to state changes.
	pub fn subscribe_state(&self) -> watch::Receiver<ServerState> {
		self.state_tx.subscribe()
	}

	/// Get the server's capabilities.
	///
	/// # Panics
	///
	/// Panics if called before initialization completes.
	pub fn capabilities(&self) -> &ServerCapabilities {
		self.capabilities
			.get()
			.expect("language server not yet initialized")
	}

	/// Get the server's capabilities if initialized.
	pub fn try_capabilities(&self) -> Option<&ServerCapabilities> {
		self.capabilities.get()
	}

	/// Check if the server supports hover.
	pub fn supports_hover(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.hover_provider.is_some())
	}

	/// Check if the server supports completion.
	pub fn supports_completion(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.completion_provider.is_some())
	}

	/// Check if the server supports formatting.
	pub fn supports_formatting(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.document_formatting_provider.is_some())
	}

	/// Check if the server supports go to definition.
	pub fn supports_definition(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.definition_provider.is_some())
	}

	/// Check if the server supports find references.
	pub fn supports_references(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.references_provider.is_some())
	}

	/// Check if the server supports document symbols.
	pub fn supports_document_symbol(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.document_symbol_provider.is_some())
	}

	/// Check if the server supports code actions.
	pub fn supports_code_action(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.code_action_provider.is_some())
	}

	/// Check if the server supports signature help.
	pub fn supports_signature_help(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.signature_help_provider.is_some())
	}

	/// Check if the server supports rename.
	pub fn supports_rename(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.rename_provider.is_some())
	}

	/// Check if the server supports execute command.
	pub fn supports_execute_command(&self) -> bool {
		self.try_capabilities()
			.is_some_and(|c| c.execute_command_provider.is_some())
	}

	/// Get the offset encoding negotiated with the server.
	pub fn offset_encoding(&self) -> OffsetEncoding {
		self.capabilities()
			.position_encoding
			.as_ref()
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

	/// Wait for the server to be ready for normal requests.
	///
	/// Returns `Ok(())` when the server transitions to `Ready` state.
	/// Returns `Err(Error::ServiceStopped)` if the server dies before becoming ready.
	pub async fn wait_ready(&self) -> crate::Result<()> {
		let mut state_rx = self.state_tx.subscribe();
		loop {
			let state = *state_rx.borrow();
			match state {
				ServerState::Ready => return Ok(()),
				ServerState::Dead => return Err(crate::Error::ServiceStopped),
				ServerState::Starting => {
					if state_rx.changed().await.is_err() {
						return Err(crate::Error::ServiceStopped);
					}
				}
			}
		}
	}

	/// Check if the server is ready (non-blocking).
	pub fn is_ready(&self) -> bool {
		*self.state_tx.borrow() == ServerState::Ready
	}

	/// Send a request to the language server.
	pub async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result> {
		let req = AnyRequest {
			id: RequestId::Number(0),
			method: R::METHOD.into(),
			params: serde_json::to_value(params).expect("Failed to serialize"),
		};
		let (tx, rx) = oneshot::channel();
		self.outbound_tx
			.send(OutboundMsg::Request {
				request: req,
				response_tx: tx,
			})
			.await
			.map_err(|_| Error::ServiceStopped)?;
		let resp = if self.timeout == Duration::ZERO {
			rx.await.map_err(|_| Error::ServiceStopped)?
		} else {
			match tokio::time::timeout(self.timeout, rx).await {
				Ok(resp) => resp.map_err(|_| Error::ServiceStopped)?,
				Err(_) => return Err(Error::RequestTimeout(R::METHOD.into())),
			}
		};
		match resp.error {
			None => Ok(serde_json::from_value(resp.result.unwrap_or_default())?),
			Some(err) => Err(Error::Response(err)),
		}
	}

	/// Send a notification to the language server.
	pub fn notify<N: Notification>(&self, params: N::Params) -> Result<()> {
		let notif = AnyNotification {
			method: N::METHOD.into(),
			params: serde_json::to_value(params).expect("Failed to serialize"),
		};
		self.send_outbound(OutboundMsg::Notification {
			notification: notif,
			barrier: None,
		})
	}

	pub(super) fn send_outbound(&self, msg: OutboundMsg) -> Result<()> {
		self.outbound_tx.try_send(msg).map_err(|err| match err {
			tokio::sync::mpsc::error::TrySendError::Closed(_) => Error::ServiceStopped,
			tokio::sync::mpsc::error::TrySendError::Full(_) => Error::Backpressure,
		})
	}
}
