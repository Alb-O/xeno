//! LSP session manager and transport event router.
//! Anchor ID: XENO_ANCHOR_LSP_MANAGER
//!
//! # Purpose
//!
//! * Define the editor-side LSP client stack: document synchronization, server registry, transport integration, and server-initiated request handling.
//! * Manage language server processes via a pluggable transport abstraction.
//!
//! # Mental model
//!
//! * `LspSystem` (in `xeno-editor`) is the editor integration root that constructs an [`crate::session::manager::LspManager`] with a transport.
//! * [`crate::sync::DocumentSync`] owns didOpen/didChange/didSave/didClose policy and the local [`crate::document::DocumentStateManager`] (diagnostics, progress).
//! * [`crate::registry::Registry`] maps `(language, workspace_root)` to a [`crate::client::ClientHandle`] and enforces singleflight for server startup.
//! * [`crate::session::manager::LspManager::spawn_router`] is the event pump that applies [`crate::client::transport::TransportEvent`] streams to [`crate::document::DocumentStateManager`] and replies to server-initiated requests in-order.
//! * [`crate::client::LocalTransport`] spawns LSP servers as child processes and manages stdin/stdout communication.
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
//! * Must singleflight `transport.start()` per `(language, root_path)` key.
//! * Must update registry indices atomically on mutation.
//! * Must process transport events sequentially and reply to requests inline.
//! * Must remove stopped/crashed servers and clear their progress.
//! * Must drop events from stale server generations.
//! * `LanguageServerId` must be slot + monotonic generation counter.
//! * `ServerConfig` must carry a pre-assigned server ID before transport start.
//! * `workspace/configuration` response must match the request item count.
//! * `workspace/workspaceFolders` response must use percent-encoded URIs.
//! * Must not send change notifications before client initialization completes.
//! * Must gate position-dependent requests on client readiness.
//! * Must return `None` for capabilities before initialization.
//! * Ready flag must require capabilities with release/acquire ordering.
//! * Must use canonicalized paths for registry lookups.
//!
//! # Data flow
//!
//! * Editor constructs `LspSystem` which constructs [`crate::session::manager::LspManager`] with [`crate::client::LocalTransport`].
//! * Editor opens a buffer; [`crate::sync::DocumentSync`] chooses a language and calls [`crate::registry::Registry::get_or_start`].
//! * [`crate::registry::Registry`] singleflights startup: assigns a [`crate::client::LanguageServerId`] (slot + generation) before
//!   calling `transport.start`, and obtains a [`crate::client::ClientHandle`] for the `(language, root_path)` key.
//! * [`crate::sync::DocumentSync`] registers the document in [`crate::document::DocumentStateManager`] and sends `didOpen` via [`crate::client::ClientHandle`].
//! * Subsequent edits call [`crate::sync::DocumentSync`] change APIs; [`crate::document::DocumentStateManager`] assigns versions; change notifications are sent and acknowledged.
//! * [`crate::client::LocalTransport`] spawns the server as a child process and communicates via stdin/stdout JSON-RPC.
//! * Transport emits [`crate::client::transport::TransportEvent`] values; [`crate::session::manager::LspManager`] router consumes them:
//!   * Generation filter: Events are checked against [`crate::registry::Registry::is_current`]; stale-generation events are dropped.
//!   * Diagnostics events update [`crate::document::DocumentStateManager`] diagnostics.
//!   * Message events: Requests are handled by `handle_server_request` and replied via `transport.reply`. Notifications update progress and may be logged.
//!   * Status events remove crashed/stopped servers from [`crate::registry::Registry`] and clear progress.
//!   * Disconnected events stop the router loop.
//!
//! # Lifecycle
//!
//! * Configuration: Editor registers [`crate::registry::LanguageServerConfig`] via [`crate::session::manager::LspManager::configure_server`].
//! * Startup: First open/change triggers [`crate::registry::Registry::get_or_start`] and transport start. Client initialization runs asynchronously; readiness is tracked by [`crate::client::ClientHandle`].
//! * Running: didOpen/didChange/didSave/didClose flow through [`crate::sync::DocumentSync`]. Router updates diagnostics/progress and services server-initiated requests.
//! * Stopped/Crashed: Transport emits status; router removes server from [`crate::registry::Registry`] and clears progress. Next operation will start a new server instance.
//!
//! # Concurrency & ordering
//!
//! * Registry startup ordering: [`crate::registry::Registry`] must ensure only one `transport.start()` runs for a given `(language, root_path)` key at a time. Waiters must block on the inflight gate and then re-check the `RegistryState`.
//! * Router ordering: [`crate::session::manager::LspManager`] router must process events in the order received from the transport receiver. Server-initiated requests must be handled inline; do not spawn per-request tasks that reorder replies.
//! * Document versioning: [`crate::document::DocumentStateManager`] versions must be monotonic per URI. When barriers are used, [`crate::sync::DocumentSync`] must only ack_change after the barrier is resolved.
//!
//! # Failure modes & recovery
//!
//! * Duplicate startup attempt: Recovery: singleflight blocks duplicates; waiters reuse the leader's handle.
//! * Server crash or stop: Recovery: router removes server; subsequent operation re-starts server via [`crate::registry::Registry`].
//! * Unsupported server-initiated request method: Recovery: handler returns `METHOD_NOT_FOUND`; add method to allowlist if required by real servers.
//! * URI conversion failure for workspaceFolders: Recovery: handler returns empty array; server may operate without workspace folders.
//!
//! # Recipes
//!
//! ## Add a new server-initiated request handler
//!
//! * Implement a method arm in `session::server_requests`.
//! * Return a stable, schema-valid JSON value for the LSP method.
//! * Ensure the handler is called inline from [`crate::session::manager::LspManager::spawn_router`].
//!
//! ## Add a new LSP feature request from the editor
//!
//! * Add a typed API method on `ClientHandle` or a controller in the `lsp` crate.
//! * Call through `DocumentSync` or a feature controller from editor code.
//! * Gate on readiness and buffer identity invariants (URI, version).
//! * Plumb results into editor UI through the existing event mechanism.
//!
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::task::JoinHandle;

use crate::client::transport::{LspTransport, TransportEvent};
use crate::{DiagnosticsEvent, DiagnosticsEventReceiver, DocumentStateManager, DocumentSync, LanguageServerConfig, Registry};

mod core;

pub use core::LspManager;
#[cfg(test)]
pub use core::SpawnRouterError;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
