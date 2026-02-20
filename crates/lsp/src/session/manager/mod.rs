//! LSP session and runtime event router.
//! Anchor ID: XENO_ANCHOR_LSP_MANAGER
//!
//! # Purpose
//!
//! * Define the editor-facing LSP client surface ([`crate::session::manager::LspSession`]) and explicit router lifecycle owner ([`crate::session::manager::LspRuntime`]).
//! * Coordinate document synchronization, server registry lifecycle, transport events, and server-initiated request handling.
//!
//! # Mental model
//!
//! * `LspSystem` (in `xeno-editor`) creates one injected `xeno_worker::WorkerRuntime`, then creates `(LspSession, LspRuntime)` and starts runtime once.
//! * [`crate::sync::DocumentSync`] owns didOpen/didChange/didSave/didClose policy and document state updates.
//! * [`crate::registry::Registry`] maps `(language, workspace_root)` to active server slots and singleflights startup.
//! * `LspRuntime` is the only transport-event subscriber. It forwards events into one supervised router actor that processes them sequentially.
//! * `LspSession` is a high-level API for editor integration (configuration, diagnostics polling, sync access).
//! * Editor-side sync orchestration (`xeno-editor::lsp::sync_manager`) is actorized and command-driven; this module remains the authoritative transport/router boundary.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | `LspSystem` (in `xeno-editor`) | Editor integration root for LSP | Must hold both session and runtime | `LspSystem::new` |
//! | [`crate::session::manager::LspSession`] | High-level client/session API | Must not own router task lifecycle | [`crate::session::manager::LspSession::new`] |
//! | [`crate::session::manager::LspRuntime`] | Router lifecycle owner | Must subscribe transport events exactly once | [`crate::session::manager::LspRuntime::start`] |
//! | [`crate::sync::DocumentSync`] | High-level document sync coordinator | Must route outbound edits through `send_change` and gate change notifications on initialization state | `DocumentSync::send_change` |
//! | [`crate::registry::Registry`] | Maps `(language, root_path)` to running clients | Must singleflight `transport.start()` per key | `Registry::acquire` |
//! | `RegistryState` | Consolidated registry indices + slot/generation tracking | Must update `servers`/`server_meta`/`id_index` atomically | `Registry::acquire`, `Registry::remove_server` |
//! | [`crate::client::LanguageServerId`] | Instance identifier: slot + generation | Generation must increase on each restart | `RegistryState::next_gen`, `ServerConfig::id` |
//! | [`crate::client::transport::TransportEvent`] | Transport-to-runtime event stream | Runtime must process sequentially | `process_transport_event` |
//! | [`crate::client::transport::TransportStatus`] | Lifecycle status events | Stopped/crashed must remove server + clear progress | `process_status_event` |
//!
//! # Invariants
//!
//! * Must singleflight `transport.start()` per `(language, root_path)` key.
//! * Must update registry indices atomically on registry mutation.
//! * Must process transport events sequentially and reply to requests inline.
//! * Must cancel router forwarding and bound runtime shutdown when transport streams remain open.
//! * Must remove stopped/crashed servers and clear their progress.
//! * Must drop events from stale server generations.
//! * `LanguageServerId` must be slot + monotonic generation counter.
//! * `ServerConfig` must carry a pre-assigned server ID before transport start.
//! * `workspace/configuration` response must match the request item count.
//! * `workspace/workspaceFolders` response must use percent-encoded URIs.
//! * Must not send change notifications before client initialization completes.
//! * Must route outbound document changes through `DocumentSync::send_change`.
//! * Must gate position-dependent requests on client readiness.
//! * Must return `None` for capabilities before initialization.
//! * Ready flag must require capabilities with release/acquire ordering.
//! * Must use canonicalized paths for registry lookups.
//! * Must execute LSP background tasks via the injected worker runtime, not ad hoc global spawns.
//! * Must clear all per-document state (diagnostics, version, opened flag) on document close.
//! * Must not resurrect opened state when late diagnostics arrive for closed documents.
//! * Must route workspace/applyEdit requests through the apply-edit channel when connected; must return not-applied when disconnected.
//! * Must return applied result from editor for workspace/applyEdit when channel is connected.
//! * Must return timeout for workspace/applyEdit when editor does not reply within the deadline.
//! * Must bound closed-document diagnostic entries and evict LRU when over the cap.
//! * Must never evict opened documents during closed-entry eviction.
//! * Must remove closed-document entry when diagnostics are cleared (empty vec).
//!
//! # Data flow
//!
//! * Editor constructs `LspSystem`, then constructs `(LspSession, LspRuntime)`.
//! * `LspRuntime::start` subscribes to the transport event stream and begins routing.
//! * Buffer open/change calls flow through [`crate::sync::DocumentSync`], which acquires servers via [`crate::registry::Registry::acquire`].
//! * Editor sync actors issue outbound document-change commands through [`crate::sync::DocumentSync::send_change`] without requiring transport event-drain polling for completion progress.
//! * Transport emits [`crate::client::transport::TransportEvent`] values; runtime routes them:
//!   * Generation filter drops stale-instance events.
//!   * Diagnostics update [`crate::document::DocumentStateManager`].
//!   * Requests route through `handle_server_request` and reply inline.
//!   * Status events remove stopped/crashed servers and clear progress.
//!   * Disconnected exits the router loop.
//!
//! # Lifecycle
//!
//! * Configuration: editor registers [`crate::registry::LanguageServerConfig`] via [`crate::session::manager::LspSession::configure_server`].
//! * Startup: first open/change acquires or starts the server in [`crate::registry::Registry`].
//! * Running: didOpen/didChange/didSave/didClose flow through [`crate::sync::DocumentSync`] (`ensure_open_text`, `send_change`, save/close helpers).
//! * Shutdown: editor first stops editor-side sync actors, then stops runtime, then stops all servers via [`crate::session::manager::LspSession::shutdown_all`].
//!
//! # Concurrency & ordering
//!
//! * Registry startup ordering: only one leader calls `transport.start()` per `(language, root_path)` key.
//! * Router ordering: a single router actor processes events in receive order. Requests are replied inline.
//! * Document versioning: pending/acked versions are monotonic and mismatch forces full sync.
//!
//! # Failure modes & recovery
//!
//! * Duplicate runtime start: `RuntimeStartError::AlreadyStarted`.
//! * Runtime start without Tokio context: `RuntimeStartError::NoRuntime`.
//! * Duplicate event subscription: surfaced by transport as protocol error.
//! * Server crash/stop: runtime removes server metadata and clears progress.
//! * Shutdown while stream open: runtime cancels event forwarding and bounds actor shutdown with graceful timeout.
//! * Unsupported server request method: returns `METHOD_NOT_FOUND`.
//!
//! # Recipes
//!
//! ## Add a server-initiated request handler
//!
//! * Add an arm in `session::server_requests::dispatch_server_request`.
//! * Keep JSON response shape schema-compatible.
//! * Ensure request handling remains inline in runtime router loop.
//!
//! ## Add a new LSP feature request
//!
//! * Add a typed method on `ClientHandle` or a feature controller.
//! * Call through `DocumentSync` or editor-facing controllers.
//! * Gate on readiness/capabilities and URI identity.
//!
//! ## Integrate with editor startup/shutdown
//!
//! * Construct `(session, runtime)` with `LspSession::new(transport, worker_runtime)`.
//! * Call `runtime.start()` from Tokio runtime context.
//! * During shutdown call editor sync-manager shutdown, then `runtime.shutdown().await`, then `session.shutdown_all().await`.

use crate::client::transport::{LspTransport, TransportEvent};
use crate::{DiagnosticsEvent, DiagnosticsEventReceiver, DocumentStateManager, DocumentSync, LanguageServerConfig, Registry};

mod core;

pub use core::{LspRuntime, LspSession, RuntimeStartError};

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod invariants;

#[cfg(test)]
mod tests;
