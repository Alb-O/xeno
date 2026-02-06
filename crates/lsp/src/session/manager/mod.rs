//! LSP session manager and transport event router.
//!
//! # Purpose
//!
//! - Define the editor-side LSP client stack: document synchronization, server registry, transport integration, and server-initiated request handling.
//! - Manage language server processes via a pluggable transport abstraction.
//!
//! # Mental model
//!
//! - [`LspSystem`](editor::lsp::system::LspSystem) is the editor integration root that constructs an [`LspManager`] with a transport.
//! - [`DocumentSync`](crate::DocumentSync) owns didOpen/didChange/didSave/didClose policy and the local [`DocumentStateManager`](crate::DocumentStateManager) (diagnostics, progress).
//! - [`Registry`](crate::Registry) maps `(language, workspace_root)` to a [`ClientHandle`](crate::ClientHandle) and enforces singleflight for server startup.
//! - [`LspManager::spawn_router`] is the event pump that applies [`TransportEvent`](crate::client::transport::TransportEvent) streams to [`DocumentStateManager`](crate::DocumentStateManager) and replies to server-initiated requests in-order.
//! - [`LocalTransport`](crate::client::LocalTransport) spawns LSP servers as child processes and manages stdin/stdout communication.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`LspSystem`] | Editor integration root for LSP | MUST construct an [`LspManager`] with a transport | `LspSystem::new` |
//! | [`LspManager`] | Owns [`DocumentSync`] and routes transport events | MUST reply to server-initiated requests inline to preserve request/reply pairing | [`LspManager::spawn_router`] |
//! | [`DocumentSync`] | High-level doc sync coordinator | MUST gate change notifications on client initialization state | `DocumentSync::*` |
//! | [`Registry`] | Maps `(language, root_path)` to a running client | MUST singleflight `transport.start()` per key | `Registry::get_or_start` |
//! | [`RegistryState`] | Consolidated registry indices + slot/generation tracking | MUST update `servers`/`server_meta`/`id_index` atomically | `Registry::get_or_start`, `Registry::remove_server` |
//! | [`LspSlotId`] | Logical server slot (language + root path pair) | Stable across restarts | `RegistryState::get_or_create_slot_id` |
//! | [`LanguageServerId`] | Instance identifier: slot + generation counter | Generation increments on each restart of the same slot | `RegistryState::next_gen`, `ServerConfig::id` |
//! | [`ServerMeta`] | Per-server metadata for server-initiated requests | MUST be removable by server id | `Registry::get_or_start`, `Registry::remove_server` |
//! | [`ClientHandle`] | RPC handle for a single language server instance | MUST NOT be treated as ready until initialization completes | `ClientHandle::*` |
//! | [`TransportEvent`] | Transport-to-manager event stream | Router MUST process sequentially | [`LspManager::spawn_router`] |
//! | [`TransportStatus`] | Lifecycle signals for server processes | Router MUST remove servers on `Stopped`/`Crashed` | [`LspManager::spawn_router`] |
//! | [`LocalTransport`] | Local child process transport | Spawns servers directly, communicates via stdin/stdout | `LocalTransport::new` |
//!
//! # Invariants
//!
//! - Registry startup MUST singleflight `transport.start()` per `(language, root_path)` key.
//!   - Enforced in: `Registry::get_or_start`
//!   - Tested by: `TODO (add regression: test_registry_singleflight_prevents_duplicate_transport_start)`
//!   - Failure symptom: duplicate server starts, leaked server processes, inconsistent server ids across callers.
//!
//! - Registry mutations MUST be atomic across `servers`, `server_meta`, and `id_index`.
//!   - Enforced in: `Registry::get_or_start`, `Registry::remove_server`
//!   - Tested by: `TODO (add regression: test_registry_remove_server_scrubs_all_indices)`
//!   - Failure symptom: stale server metadata persists after removal, status cleanup fails to fully detach, server request handlers read wrong settings/root.
//!
//! - The router MUST process transport events sequentially and MUST reply to server-initiated requests inline.
//!   - Enforced in: [`LspManager::spawn_router`]
//!   - Tested by: `TODO (add regression: test_router_event_ordering)`
//!   - Failure symptom: server request/reply pairing breaks, replies go to the wrong pending request, server-side hangs waiting for a response.
//!   - Note: Cleanup operations (like `transport.stop` on crash) are spawned as fire-and-forget tasks to prevent blocking.
//!
//! - On `TransportStatus::Stopped` or `TransportStatus::Crashed`, the router MUST remove the server from `Registry` and MUST clear per-server progress.
//!   - Enforced in: [`LspManager::spawn_router`], `Registry::remove_server`
//!   - Tested by: `TODO (add regression: test_status_stopped_removes_server_and_clears_progress)`
//!   - Failure symptom: UI shows stuck progress forever, stale `ClientHandle` remains reachable, subsequent requests wedge on a dead server id.
//!
//! - The router MUST discard transport events from stale server generations.
//!   - Enforced in: [`LspManager::spawn_router`] (checks `Registry::is_current` before dispatching)
//!   - Tested by: TODO (add regression: test_router_drops_stale_generation_events)
//!   - Failure symptom: Diagnostics or progress from a previous server generation leak into the current session, causing phantom diagnostics or stuck progress indicators.
//!
//! - `LanguageServerId` MUST be a (slot, generation) pair. The slot is stable for a given `(language, root_path)` key; the generation increments on each restart.
//!   - Enforced in: `RegistryState::get_or_create_slot_id`, `RegistryState::next_gen`
//!   - Tested by: TODO (add regression: test_server_id_generation_increments_on_restart)
//!   - Failure symptom: Events from a restarted server are misattributed to the previous instance, or `is_current` check always passes for stale IDs.
//!
//! - `ServerConfig` MUST carry the pre-assigned `LanguageServerId`; the transport MUST NOT generate its own IDs.
//!   - Enforced in: `Registry::get_or_start` (assigns ID before calling `transport.start`), `LocalTransport::start` (uses `cfg.id`)
//!   - Tested by: `lsp::registry::tests::test_singleflight_start` (asserts `StartedServer { id: cfg.id }`)
//!   - Failure symptom: Transport-assigned IDs diverge from registry IDs, breaking event routing and `is_current` checks.
//!
//! - `workspace/configuration` handling MUST return an array with one element per requested item, and MUST return an object for missing config.
//!   - Enforced in: `handle_workspace_configuration`
//!   - Tested by: `TODO (add regression: test_server_request_workspace_configuration_section_slicing)`
//!   - Failure symptom: servers treat configuration as invalid, disable features, or log repeated configuration query errors.
//!
//! - `workspace/workspaceFolders` handling MUST return percent-encoded file URIs.
//!   - Enforced in: `handle_workspace_folders`
//!   - Tested by: `TODO (add regression: test_server_request_workspace_folders_uri_encoding)`
//!   - Failure symptom: servers mis-parse the workspace root for paths with spaces or non-ASCII characters and degrade indexing/navigation.
//!
//! - `DocumentSync` MUST NOT send change notifications before the client has completed initialization.
//!   - Enforced in: `DocumentSync::notify_change_full_text`, `DocumentSync::notify_change_incremental_no_content`
//!   - Tested by: `lsp::sync::tests::test_document_sync_returns_not_ready_before_init`
//!   - Failure symptom: edits are dropped by the server or applied out of order, resulting in stale diagnostics and incorrect completions.
//!
//! - `LspSystem::prepare_position_request` MUST gate on `ClientHandle::is_ready()` before forming any position-based LSP request.
//!   - Enforced in: `LspSystem::prepare_position_request`
//!   - Tested by: `TODO (add regression: test_prepare_position_request_returns_none_before_ready)`
//!   - Failure symptom: requests sent to uninitialized servers cause panics or silent errors.
//!
//! - `ClientHandle::capabilities()` MUST return `Option` (not panic). All capability-dependent public methods MUST use the fallible accessor.
//!   - Enforced in: `ClientHandle::capabilities`, `ClientHandle::offset_encoding`, `ClientHandle::supports_*`
//!   - Tested by: `TODO (add regression: test_client_handle_capabilities_returns_none_before_init)`
//!   - Failure symptom: panic ("language server not yet initialized") on any code path that reads capabilities before the initialize handshake completes.
//!
//! - `ClientHandle::set_ready(true)` MUST only be called after `capabilities.set()` and MUST use `Release` ordering. `is_ready()` MUST use `Acquire` ordering.
//!   - Enforced in: `ClientHandle::set_ready` (`debug_assert` + `Release`), `ClientHandle::is_ready` (`Acquire`)
//!   - Tested by: `TODO (add regression: test_set_ready_requires_initialized)`
//!   - Failure symptom: thread observes `is_ready() == true` but `capabilities()` returns `None` due to missing memory ordering edge.
//!
//! - All registry lookups in `LspSystem` MUST use canonicalized paths to match the key representation used at registration time.
//!   - Enforced in: `LspSystem::prepare_position_request`, `LspSystem::offset_encoding_for_buffer`, `LspSystem::incremental_encoding`
//!   - Tested by: `TODO (add regression: test_registry_lookup_uses_canonical_path)`
//!   - Failure symptom: registry miss on symlinked or relative paths causes silent fallback to wrong default encoding (UTF-16) or drops the request entirely.
//!
//! # Data flow
//!
//! - Editor constructs `LspSystem` which constructs `LspManager` with `LocalTransport`.
//! - Editor opens a buffer; `DocumentSync` chooses a language and calls `Registry::get_or_start(language, path)`.
//! - `Registry` singleflights startup: assigns a `LanguageServerId` (slot + generation) before
//!   calling `transport.start`, and obtains a `ClientHandle` for the `(language, root_path)` key.
//! - `DocumentSync` registers the document in `DocumentStateManager` and sends `didOpen` via `ClientHandle`.
//! - Subsequent edits call `DocumentSync` change APIs; `DocumentStateManager` assigns versions; change notifications are sent and acknowledged.
//! - `LocalTransport` spawns the server as a child process and communicates via stdin/stdout JSON-RPC.
//! - Transport emits `TransportEvent` values; `LspManager` router consumes them:
//!   - Generation filter: Events are checked against `Registry::is_current`; stale-generation events are dropped.
//!   - Diagnostics events update `DocumentStateManager` diagnostics.
//!   - Message events: Requests are handled by `handle_server_request` and replied via `transport.reply`. Notifications update progress and may be logged.
//!   - Status events remove crashed/stopped servers from `Registry` and clear progress.
//!   - Disconnected events stop the router loop.
//!
//! # Lifecycle
//!
//! - Configuration: Editor registers `LanguageServerConfig` via `LspManager::configure_server`.
//! - Startup: First open/change triggers `Registry::get_or_start` and transport start. Client initialization runs asynchronously; readiness is tracked by `ClientHandle`.
//! - Running: didOpen/didChange/didSave/didClose flow through `DocumentSync`. Router updates diagnostics/progress and services server-initiated requests.
//! - Stopped/Crashed: Transport emits status; router removes server from `Registry` and clears progress. Next operation will start a new server instance.
//!
//! # Concurrency and ordering
//!
//! - Registry startup ordering: `Registry` MUST ensure only one `transport.start()` runs for a given `(language, root_path)` key at a time. Waiters MUST block on the inflight gate and then re-check the `RegistryState`.
//! - Router ordering: `LspManager` router MUST process events in the order received from the transport receiver. Server-initiated requests MUST be handled inline; do not spawn per-request tasks that reorder replies.
//! - Document versioning: `DocumentStateManager` versions MUST be monotonic per URI. When barriers are used, `DocumentSync` MUST only ack_change after the barrier is resolved.
//!
//! # Failure modes and recovery
//!
//! - Duplicate startup attempt: Recovery: singleflight blocks duplicates; waiters reuse the leader's handle.
//! - Server crash or stop: Recovery: router removes server; subsequent operation re-starts server via `Registry`.
//! - Unsupported server-initiated request method: Recovery: handler returns `METHOD_NOT_FOUND`; add method to allowlist if required by real servers.
//! - URI conversion failure for workspaceFolders: Recovery: handler returns empty array; server may operate without workspace folders.
//!
//! # Recipes
//!
//! ## Add a new server-initiated request handler
//!
//! - Implement a method arm in `session::server_requests`.
//! - Return a stable, schema-valid JSON value for the LSP method.
//! - Ensure the handler is called inline from [`LspManager::spawn_router`].
//! - Add a regression test: `TODO (add regression: test_server_request_<method_name>)`.
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
mod tests;
