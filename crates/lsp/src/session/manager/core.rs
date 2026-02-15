use super::*;

#[derive(Debug, thiserror::Error)]
pub enum SpawnRouterError {
	#[error("router already started")]
	AlreadyStarted,
	#[error("no tokio runtime available")]
	NoRuntime,
}

/// Central manager for LSP functionality.
pub struct LspManager {
	sync: DocumentSync,
	diagnostics_receiver: Option<DiagnosticsEventReceiver>,
	transport: Arc<dyn LspTransport>,
	router_started: AtomicBool,
}

impl LspManager {
	/// Create a new LSP manager with the given transport.
	pub fn new(transport: Arc<dyn LspTransport>) -> Self {
		let (sync, _registry, _documents, diagnostics_receiver) = DocumentSync::create(transport.clone());

		Self {
			sync,
			diagnostics_receiver: Some(diagnostics_receiver),
			transport,
			router_started: AtomicBool::new(false),
		}
	}

	/// Spawn the background event router task.
	///
	/// Routes transport events to document state and handles server-initiated requests.
	/// Must be called from within a Tokio runtime.
	pub fn spawn_router(&self) -> Result<JoinHandle<()>, SpawnRouterError> {
		// Must be called within a Tokio runtime.
		if tokio::runtime::Handle::try_current().is_err() {
			return Err(SpawnRouterError::NoRuntime);
		}

		// Enforce single router instance per LspManager.
		if self.router_started.swap(true, Ordering::SeqCst) {
			return Err(SpawnRouterError::AlreadyStarted);
		}

		let mut events_rx = self.transport.events();
		let documents = self.sync.documents_arc();
		let transport = self.transport.clone();
		let sync = self.sync.clone();

		Ok(tokio::spawn(async move {
			while let Some(event) = events_rx.recv().await {
				let server_id = match &event {
					TransportEvent::Diagnostics { server, .. } => Some(*server),
					TransportEvent::Message { server, .. } => Some(*server),
					TransportEvent::Status { server, .. } => Some(*server),
					TransportEvent::Disconnected => None,
				};

				// Drop events from stale server generations.
				if let Some(id) = server_id
					&& !sync.registry().is_current(id)
				{
					tracing::debug!(
						server_id = %id,
						"Dropping event from stale server instance"
					);
					continue;
				}

				match event {
					TransportEvent::Diagnostics {
						server: _,
						uri,
						version,
						diagnostics,
					} => {
						let Ok(uri) = uri.parse::<lsp_types::Uri>() else {
							continue;
						};
						let Ok(diags) = serde_json::from_value::<Vec<lsp_types::Diagnostic>>(diagnostics) else {
							continue;
						};

						documents.update_diagnostics(&uri, diags, version.and_then(|v| i32::try_from(v).ok()));
					}

					TransportEvent::Message { server, message } => {
						use crate::Message;

						match message {
							Message::Request(req) => {
								tracing::debug!(
									server_id = %server,
									method = %req.method,
									"Handling server request"
								);
								let req_id = req.id.clone();

								let result = crate::session::server_requests::handle_server_request(&sync, server, req).await;

								if let Err(e) = transport.reply(server, req_id, result).await {
									tracing::error!(
										server_id = %server,
										error = ?e,
										"Failed to reply to server request"
									);
								}
							}

							Message::Notification(notif) => {
								if notif.method == "$/progress" {
									if let Ok(params) = serde_json::from_value::<lsp_types::ProgressParams>(notif.params) {
										documents.update_progress(server, params);
									}
								} else if notif.method == "window/logMessage" || notif.method == "window/showMessage" {
									tracing::debug!(
										server_id = %server,
										method = %notif.method,
										"Server notification"
									);
								}
							}

							Message::Response(_) => {}
						}
					}

					TransportEvent::Status { server, status } => {
						use crate::client::transport::TransportStatus;

						match status {
							TransportStatus::Stopped | TransportStatus::Crashed => {
								// Remove server state from Registry.
								if let Some(meta) = sync.registry().remove_server(server) {
									tracing::warn!(
										server_id = %server,
										language = %meta.language,
										status = ?status,
										"LSP server stopped, removed from registry"
									);
								}

								// Stop transport asynchronously (don’t block router loop).
								let transport_clone = transport.clone();
								tokio::spawn(async move {
									let _ = transport_clone.stop(server).await;
								});

								// Clear per-server progress.
								documents.clear_server_progress(server);
							}

							TransportStatus::Starting | TransportStatus::Running => {
								tracing::debug!(
									server_id = %server,
									status = ?status,
									"LSP server status update"
								);
							}
						}
					}

					TransportEvent::Disconnected => break,
				}
			}
		}))
	}

	/// Create an LSP manager with existing registry and document state.
	pub fn with_state(registry: Arc<Registry>, documents: Arc<DocumentStateManager>) -> Self {
		let transport = registry.transport();
		let sync = DocumentSync::with_registry(registry, documents);
		Self {
			sync,
			diagnostics_receiver: None,
			transport,
			router_started: AtomicBool::new(false),
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
		let ids = self.sync.registry().shutdown_all();
		for id in ids {
			let _ = self.transport.stop(id).await;
		}
	}
}
