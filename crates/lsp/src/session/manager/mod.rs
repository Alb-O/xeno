//! LSP session manager and transport event router.
//!
//! # Purpose
//!
//! - Define the editor-side LSP client stack: document synchronization, server registry, transport integration, and server-initiated request handling.
//! - Manage language server processes via a pluggable transport abstraction.
//!
//! # Mental model
//!
//! - `LspSystem` (in `xeno-editor`) is the editor integration root that constructs an [`crate::session::manager::LspManager`] with a transport.
//! - [`crate::sync::DocumentSync`] owns didOpen/didChange/didSave/didClose policy and the local [`crate::document::DocumentStateManager`] (diagnostics, progress).
//! - [`crate::registry::Registry`] maps `(language, workspace_root)` to a [`crate::client::ClientHandle`] and enforces singleflight for server startup.
//! - [`crate::session::manager::LspManager::spawn_router`] is the event pump that applies [`crate::client::transport::TransportEvent`] streams to [`crate::document::DocumentStateManager`] and replies to server-initiated requests in-order.
//! - [`crate::client::LocalTransport`] spawns LSP servers as child processes and manages stdin/stdout communication.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | `LspSystem` (in `xeno-editor`) | Editor integration root for LSP | Must construct an `LspManager` with a transport | `LspSystem::new` |
//! | [`crate::session::manager::LspManager`] | Owns [`crate::sync::DocumentSync`] and routes transport events | Must reply to server-initiated requests inline to preserve request/reply pairing | [`crate::session::manager::LspManager::spawn_router`] |
//! | [`crate::sync::DocumentSync`] | High-level doc sync coordinator | Must gate change notifications on client initialization state | `DocumentSync::*` |
//! | [`crate::registry::Registry`] | Maps `(language, root_path)` to a running client | Must singleflight `transport.start()` per key | `Registry::get_or_start` |
//! | `RegistryState` | Consolidated registry indices + slot/generation tracking | Must update `servers`/`server_meta`/`id_index` atomically | `Registry::get_or_start`, `Registry::remove_server` |
//! | [`crate::client::LspSlotId`] | Logical server slot (language + root path pair) | Stable across restarts | `RegistryState::get_or_create_slot_id` |
//! | [`crate::client::LanguageServerId`] | Instance identifier: slot + generation counter | Generation increments on each restart of the same slot | `RegistryState::next_gen`, `ServerConfig::id` |
//! | [`crate::registry::ServerMeta`] | Per-server metadata for server-initiated requests | Must be removable by server id | `Registry::get_or_start`, `Registry::remove_server` |
//! | [`crate::client::ClientHandle`] | RPC handle for a single language server instance | Must not be treated as ready until initialization completes | `ClientHandle::*` |
//! | [`crate::client::transport::TransportEvent`] | Transport-to-manager event stream | Router must process sequentially | [`crate::session::manager::LspManager::spawn_router`] |
//! | [`crate::client::transport::TransportStatus`] | Lifecycle signals for server processes | Router must remove servers on `Stopped`/`Crashed` | [`crate::session::manager::LspManager::spawn_router`] |
//! | [`crate::client::LocalTransport`] | Local child process transport | Spawns servers directly, communicates via stdin/stdout | `LocalTransport::new` |
//!
//! # Invariants
//!
//! - Must singleflight `transport.start()` per `(language, root_path)` key.
//! - Must update registry indices atomically on mutation.
//! - Must process transport events sequentially and reply to requests inline.
//! - Must remove stopped/crashed servers and clear their progress.
//! - Must drop events from stale server generations.
//! - `LanguageServerId` must be slot + monotonic generation counter.
//! - `ServerConfig` must carry a pre-assigned server ID before transport start.
//! - `workspace/configuration` response must match the request item count.
//! - `workspace/workspaceFolders` response must use percent-encoded URIs.
//! - Must not send change notifications before client initialization completes.
//! - Must gate position-dependent requests on client readiness.
//! - Must return `None` for capabilities before initialization.
//! - Ready flag must require capabilities with release/acquire ordering.
//! - Must use canonicalized paths for registry lookups.
//!
//! # Data flow
//!
//! - Editor constructs `LspSystem` which constructs [`crate::session::manager::LspManager`] with [`crate::client::LocalTransport`].
//! - Editor opens a buffer; [`crate::sync::DocumentSync`] chooses a language and calls [`crate::registry::Registry::get_or_start`].
//! - [`crate::registry::Registry`] singleflights startup: assigns a [`crate::client::LanguageServerId`] (slot + generation) before
//!   calling `transport.start`, and obtains a [`crate::client::ClientHandle`] for the `(language, root_path)` key.
//! - [`crate::sync::DocumentSync`] registers the document in [`crate::document::DocumentStateManager`] and sends `didOpen` via [`crate::client::ClientHandle`].
//! - Subsequent edits call [`crate::sync::DocumentSync`] change APIs; [`crate::document::DocumentStateManager`] assigns versions; change notifications are sent and acknowledged.
//! - [`crate::client::LocalTransport`] spawns the server as a child process and communicates via stdin/stdout JSON-RPC.
//! - Transport emits [`crate::client::transport::TransportEvent`] values; [`crate::session::manager::LspManager`] router consumes them:
//!   - Generation filter: Events are checked against [`crate::registry::Registry::is_current`]; stale-generation events are dropped.
//!   - Diagnostics events update [`crate::document::DocumentStateManager`] diagnostics.
//!   - Message events: Requests are handled by `handle_server_request` and replied via `transport.reply`. Notifications update progress and may be logged.
//!   - Status events remove crashed/stopped servers from [`crate::registry::Registry`] and clear progress.
//!   - Disconnected events stop the router loop.
//!
//! # Lifecycle
//!
//! - Configuration: Editor registers [`crate::registry::LanguageServerConfig`] via [`crate::session::manager::LspManager::configure_server`].
//! - Startup: First open/change triggers [`crate::registry::Registry::get_or_start`] and transport start. Client initialization runs asynchronously; readiness is tracked by [`crate::client::ClientHandle`].
//! - Running: didOpen/didChange/didSave/didClose flow through [`crate::sync::DocumentSync`]. Router updates diagnostics/progress and services server-initiated requests.
//! - Stopped/Crashed: Transport emits status; router removes server from [`crate::registry::Registry`] and clears progress. Next operation will start a new server instance.
//!
//! # Concurrency and ordering
//!
//! - Registry startup ordering: [`crate::registry::Registry`] must ensure only one `transport.start()` runs for a given `(language, root_path)` key at a time. Waiters must block on the inflight gate and then re-check the `RegistryState`.
//! - Router ordering: [`crate::session::manager::LspManager`] router must process events in the order received from the transport receiver. Server-initiated requests must be handled inline; do not spawn per-request tasks that reorder replies.
//! - Document versioning: [`crate::document::DocumentStateManager`] versions must be monotonic per URI. When barriers are used, [`crate::sync::DocumentSync`] must only ack_change after the barrier is resolved.
//!
//! # Failure modes and recovery
//!
//! - Duplicate startup attempt: Recovery: singleflight blocks duplicates; waiters reuse the leader's handle.
//! - Server crash or stop: Recovery: router removes server; subsequent operation re-starts server via [`crate::registry::Registry`].
//! - Unsupported server-initiated request method: Recovery: handler returns `METHOD_NOT_FOUND`; add method to allowlist if required by real servers.
//! - URI conversion failure for workspaceFolders: Recovery: handler returns empty array; server may operate without workspace folders.
//!
//! # Recipes
//!
//! ## Add a new server-initiated request handler
//!
//! - Implement a method arm in `session::server_requests`.
//! - Return a stable, schema-valid JSON value for the LSP method.
//! - Ensure the handler is called inline from [`crate::session::manager::LspManager::spawn_router`].
//!
//! ## Add a new LSP feature request from the editor
//!
//! - Add a typed API method on `ClientHandle` or a controller in the `lsp` crate.
//! - Call through `DocumentSync` or a feature controller from editor code.
//! - Gate on readiness and buffer identity invariants (URI, version).
//! - Plumb results into editor UI through the existing event mechanism.
//!
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::task::JoinHandle;

use crate::client::transport::{LspTransport, TransportEvent};
use crate::{
	DiagnosticsEvent, DiagnosticsEventReceiver, DocumentStateManager, DocumentSync,
	LanguageServerConfig, Registry,
};

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
		let (sync, _registry, _documents, diagnostics_receiver) =
			DocumentSync::create(transport.clone());

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
						let Ok(diags) =
							serde_json::from_value::<Vec<lsp_types::Diagnostic>>(diagnostics)
						else {
							continue;
						};

						documents.update_diagnostics(
							&uri,
							diags,
							version.and_then(|v| i32::try_from(v).ok()),
						);
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

								let result = super::server_requests::handle_server_request(
									&sync, server, req,
								)
								.await;

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
									if let Ok(params) = serde_json::from_value::<
										lsp_types::ProgressParams,
									>(notif.params)
									{
										documents.update_progress(server, params);
									}
								} else if notif.method == "window/logMessage"
									|| notif.method == "window/showMessage"
								{
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

// Default implementation removed: LspManager requires an explicit transport.
// Use LocalTransport::new() to create a transport that spawns servers locally.
// Users should construct LspManager via LspSystem::new() in editor code.

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
