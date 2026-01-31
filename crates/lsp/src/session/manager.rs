use std::sync::Arc;

use crate::client::transport::{LspTransport, TransportEvent};
use crate::{
	DiagnosticsEvent, DiagnosticsEventReceiver, DocumentStateManager, DocumentSync,
	LanguageServerConfig, Registry,
};

/// Central manager for LSP functionality.
pub struct LspManager {
	sync: DocumentSync,
	diagnostics_receiver: Option<DiagnosticsEventReceiver>,
	transport: Arc<dyn LspTransport>,
}

impl LspManager {
	/// Create a new LSP manager with the given transport.
	pub fn new(transport: Arc<dyn LspTransport>) -> Self {
		let (sync, _registry, _documents, diagnostics_receiver) =
			DocumentSync::create(transport.clone());

		Self {
			sync,
			diagnostics_receiver: Some(diagnostics_receiver),
			transport,
		}
	}

	/// Spawn the background event router task.
	///
	/// Routes transport events to document state and handles server-initiated requests.
	/// Must be called from within a Tokio runtime.
	pub fn spawn_router(&self) {
		if tokio::runtime::Handle::try_current().is_err() {
			return;
		}
		let events_rx = self.transport.events();
		let documents_clone = self.sync.documents_arc();
		let transport = self.transport.clone();
		let sync_clone = self.sync.clone();

		tokio::spawn(async move {
			let mut events_rx = events_rx;
			while let Some(event) = events_rx.recv().await {
				match event {
					TransportEvent::Diagnostics {
						server: _,
						uri,
						version,
						diagnostics,
					} => {
						if let Ok(uri) = uri.parse::<lsp_types::Uri>()
							&& let Ok(diags) =
								serde_json::from_value::<Vec<lsp_types::Diagnostic>>(diagnostics)
						{
							documents_clone.update_diagnostics(
								&uri,
								diags,
								version.map(|v| v as i32),
							);
						}
					}
					TransportEvent::Message { server, message } => {
						use crate::Message;

						match message {
							Message::Request(req) => {
								tracing::debug!(server_id = server.0, method = %req.method, "Handling server request");
								let result = super::server_requests::handle_server_request(
									&sync_clone,
									server,
									req,
								)
								.await;
								if let Err(e) = transport.reply(server, result).await {
									tracing::error!(server_id = server.0, error = ?e, "Failed to reply to server request");
								}
							}
							Message::Notification(notif) => {
								if notif.method == "$/progress" {
									if let Ok(params) = serde_json::from_value::<
										lsp_types::ProgressParams,
									>(notif.params)
									{
										documents_clone.update_progress(server, params);
									}
								} else if notif.method == "window/logMessage"
									|| notif.method == "window/showMessage"
								{
									tracing::debug!(server_id = server.0, method = %notif.method, "Server notification");
								}
							}
							Message::Response(_) => {}
						}
					}
					TransportEvent::Status { server, status } => {
						use crate::client::transport::TransportStatus;

						match status {
							TransportStatus::Stopped | TransportStatus::Crashed => {
								// Clean up registry state for crashed/stopped servers
								if let Some(meta) = sync_clone.registry().remove_server(server) {
									tracing::warn!(
										server_id = server.0,
										language = %meta.language,
										status = ?status,
										"LSP server stopped, removed from registry"
									);
								}
								// Clear any progress indicators for this server
								documents_clone.clear_server_progress(server);
							}
							TransportStatus::Starting | TransportStatus::Running => {
								// Status updates - currently just logged
								tracing::debug!(server_id = server.0, status = ?status, "LSP server status update");
							}
						}
					}
					TransportEvent::Disconnected => break,
				}
			}
		});
	}

	/// Create an LSP manager with existing registry and document state.
	pub fn with_state(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		let transport = registry.transport();
		let sync = DocumentSync::with_registry(registry, documents);
		Self {
			sync,
			diagnostics_receiver: None,
			transport,
		}
	}

	/// Poll for pending diagnostic events.
	pub fn poll_diagnostics(&mut self) -> Vec<DiagnosticsEvent> {
		let Some(ref mut receiver) = self.diagnostics_receiver else {
			return Vec::new();
		};

		let mut events = Vec::new();
		while let Ok(event) = receiver.try_recv() {
			events.push(event);
		}
		events
	}

	/// Get the diagnostics version counter.
	pub fn diagnostics_version(&self) -> u64 {
		self.sync.documents().diagnostics_version()
	}

	/// Configure a language server.
	pub fn configure_server(&self, language: impl Into<String>, config: LanguageServerConfig) {
		self.sync.registry().register(language, config);
	}

	/// Remove a language server configuration.
	pub fn remove_server(&self, language: &str) {
		self.sync.registry().unregister(language);
	}

	/// Get the document sync coordinator.
	pub fn sync(&self) -> &DocumentSync {
		&self.sync
	}

	/// Get the server registry.
	pub fn registry(&self) -> &Registry {
		self.sync.registry()
	}

	/// Get the document state manager.
	pub fn documents(&self) -> &DocumentStateManager {
		self.sync.documents()
	}

	/// Shutdown all language servers.
	pub async fn shutdown_all(&self) {
		self.sync.registry().shutdown_all().await;
	}
}

// Default implementation removed: LspManager requires an explicit transport.
// Broker transport is now the standard, but cannot be constructed here due to
// crate boundaries. Users must construct LspManager via LspSystem::new().

#[cfg(test)]
mod tests {
	use async_trait::async_trait;
	use serde_json::Value as JsonValue;
	use tokio::sync::{mpsc, oneshot};

	use super::*;
	use crate::client::LanguageServerId;
	use crate::types::{AnyNotification, AnyRequest, AnyResponse, ResponseError};

	/// Minimal stub transport for testing
	struct StubTransport;

	#[async_trait]
	impl LspTransport for StubTransport {
		fn events(&self) -> mpsc::UnboundedReceiver<TransportEvent> {
			let (_, rx) = mpsc::unbounded_channel();
			rx
		}

		async fn start(
			&self,
			_cfg: crate::client::ServerConfig,
		) -> crate::Result<crate::client::transport::StartedServer> {
			Err(crate::Error::Protocol("StubTransport".into()))
		}

		async fn notify(
			&self,
			_server: LanguageServerId,
			_notif: AnyNotification,
		) -> crate::Result<()> {
			Ok(())
		}

		async fn notify_with_barrier(
			&self,
			_server: LanguageServerId,
			_notif: AnyNotification,
		) -> crate::Result<oneshot::Receiver<()>> {
			let (tx, rx) = oneshot::channel();
			let _ = tx.send(());
			Ok(rx)
		}

		async fn request(
			&self,
			_server: LanguageServerId,
			_req: AnyRequest,
			_timeout: Option<std::time::Duration>,
		) -> crate::Result<AnyResponse> {
			Err(crate::Error::Protocol("StubTransport".into()))
		}

		async fn reply(
			&self,
			_server: LanguageServerId,
			_resp: Result<JsonValue, ResponseError>,
		) -> crate::Result<()> {
			Ok(())
		}
	}

	#[test]
	fn test_lsp_manager_creation() {
		let transport: Arc<dyn LspTransport> = Arc::new(StubTransport);
		let manager = LspManager::new(transport);
		assert_eq!(manager.diagnostics_version(), 0);
	}
}
