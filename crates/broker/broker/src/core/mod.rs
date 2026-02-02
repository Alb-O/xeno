//! Broker daemon for LSP server deduplication, routing, and buffer synchronization.
//!
//! # Purpose
//!
//! - Define the broker daemon that deduplicates, shares, and supervises language server processes across editor sessions.
//! - Describe broker-side routing rules for server-to-client JSON-RPC, including leader election, pending request tracking, and lease-based persistence.
//! - Define cross-process buffer synchronization: single-writer model where the broker maintains an authoritative rope, the owner session publishes deltas, and the broker validates, applies, and broadcasts to follower sessions.
//! - Exclude editor-side document sync and UI integration; see `lsp::session::manager` module docs.
//!
//! # Mental model
//!
//! - The broker is an out-of-process daemon that owns the actual LSP server processes.
//! - Editor sessions connect to the broker via IPC and register a [`SessionId`].
//! - Each LSP server instance is keyed by [`ProjectKey`] and shared across sessions that attach to that server.
//! - Server-to-client requests are routed only to the leader session (deterministic: minimum [`SessionId`]).
//! - Client-to-server requests are rewritten to broker-allocated wire request ids to avoid collisions between sessions.
//! - The broker keeps idle servers alive for an idle lease duration; after lease expiry and with no inflight requests, the server is terminated.
//! - BrokerCore splits routing and buffer sync state behind independent locks; no code path may hold both at once.
//! - For buffer sync: each document URI has exactly one owner session. The owner sends deltas; the broker validates epoch/sequence, applies to its authoritative rope, and broadcasts to all other sessions (followers). Ownership transfers on disconnect or explicit request, bumping the epoch and resetting the sequence.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`BrokerCore`] | Authoritative broker state machine | MUST be the only owner of session/server maps | `BrokerCore::*` |
//! | [`RoutingState`] | Routing-only broker state | MUST only be accessed under the routing lock and never across IPC sends | `BrokerCore::*` |
//! | [`SyncState`] | Buffer sync broker state | MUST only be accessed under the sync lock and never across `rope.to_string()` | `BrokerCore::on_buffer_sync_*` |
//! | [`ProjectKey`] | Dedup key for LSP servers | MUST uniquely represent command/args/cwd (with no-cwd sentinel) | `ProjectKey::from` |
//! | [`ServerEntry`] | One managed LSP server instance | MUST maintain leader = min(attached) | `BrokerCore::attach_session`, `BrokerCore::detach_session` |
//! | [`SessionEntry`] | One connected editor session | MUST track attachment set for cleanup | `BrokerCore::register_session`, `BrokerCore::unregister_session` |
//! | [`PendingS2cReq`] | Pending server-to-client request | MUST be completed only by the elected responder | `BrokerCore::register_client_request`, `BrokerCore::complete_client_request` |
//! | [`PendingC2sReq`] | Pending client-to-server request | MUST track origin session and original request id | `BrokerCore::*` |
//! | [`LspProxyService`] | LSP stdio proxy and event forwarder | MUST register pending before forwarding request | `LspProxyService::call`, `LspProxyService::forward` |
//! | [`DocRegistry`] | URI to (DocId, version) tracking | MUST not report a doc that is not in `by_uri` | `DocRegistry::update`, `BrokerCore::get_doc_by_uri` |
//! | [`DocOwnerRegistry`] | Single-writer ownership per URI | MUST transfer ownership on detach/unregister | `BrokerCore::cleanup_session_docs_on_server` |
//! | [`SyncDocState`] | Per-URI broker-authoritative sync state | MUST have exactly one owner; epoch increments on ownership change; seq increments on delta; owner must resync before publishing after transfer | `BrokerCore::on_buffer_sync_open`, `BrokerCore::on_buffer_sync_delta`, `BrokerCore::on_buffer_sync_close` |
//! | [`SyncEpoch`] | Monotonic ownership generation | MUST increment on every ownership transfer | `BrokerCore::on_buffer_sync_take_ownership`, `BrokerCore::on_buffer_sync_close` |
//! | [`SyncSeq`] | Monotonic edit sequence within an epoch | MUST increment on every applied delta; resets to 0 on epoch change | `BrokerCore::on_buffer_sync_delta` |
//! | [`KnowledgeCore`] | Persistent workspace search index | MUST own a dedicated helix-db instance | `KnowledgeCore::open` |
//! | [`knowledge::crawler::ProjectCrawler`] | Background filesystem indexer for unopened files | MUST skip open sync docs and unchanged mtimes | `ProjectCrawler::spawn` |
//! | [`BufferSyncManager`] | Editor-side per-document sync tracker | MUST clear all state on broker disconnect | `BufferSyncManager::disable_all` |
//! | [`Event`] | Broker-to-editor event stream | MUST include `server_id` for routing on client side; buffer sync events MUST include URI | `BrokerCore::broadcast_to_server`, `BrokerCore::send_to_leader`, `BrokerCore::broadcast_to_sync_doc_sessions` |
//!
//! # Invariants
//!
//! 1. Project deduplication MUST use a stable [`ProjectKey`]; configs without cwd MUST not collapse unrelated projects.
//!    - Enforced in: `ProjectKey::from`
//!    - Tested by: `core::tests::project_dedup_*`
//!    - Failure symptom: unrelated projects share a server, causing incorrect diagnostics and cross-project symbol results.
//!
//! 2. Leader election MUST be deterministic and MUST be the minimum [`SessionId`] of the attached set.
//!    - Enforced in: `BrokerCore::attach_session`, `BrokerCore::detach_session`
//!    - Tested by: `test_broker_e2e_leader_routing_and_reply`
//!    - Failure symptom: server-initiated requests route to different sessions across runs, breaking request handling and causing hangs.
//!
//! 3. Server-to-client requests MUST be registered as pending before being forwarded to the leader session.
//!    - Enforced in: `LspProxyService::call`
//!    - Tested by: `core::tests::request_routing::reply_from_leader_completes_pending`
//!    - Failure symptom: leader reply arrives before pending registration and is rejected as "request not found".
//!
//! 4. Server-to-client requests MUST only be completed by the elected responder session.
//!    - Enforced in: `BrokerCore::complete_client_request`
//!    - Tested by: `test_broker_e2e_leader_routing_and_reply`
//!    - Failure symptom: replies are accepted from non-leader sessions, resulting in nondeterministic behavior and incorrect responses.
//!
//! 5. Client-to-server request ids MUST be rewritten to broker-allocated wire ids to prevent cross-session collisions.
//!    - Enforced in: `BrokerCore::alloc_wire_request_id`
//!    - Tested by: `test_broker_string_wire_ids`
//!    - Failure symptom: one session's response completes another session's request, causing incorrect editor UI and protocol errors.
//!
//! 6. Pending requests MUST be cancelled on leader change, session unregister, server exit, and per-request timeout.
//!    - Enforced in: `BrokerCore::cancel_pending_for_leader_change`, `BrokerCore::unregister_session`, `BrokerCore::check_lease_expiry`, `LspProxyService::call`
//!    - Tested by: `core::tests::request_routing::disconnect_leader_cancels_pending_requests`
//!    - Failure symptom: pending maps leak, late replies are misdelivered, or server waits forever for a client reply.
//!
//! 7. IPC send failure to a session MUST trigger authoritative session cleanup.
//!    - Enforced in: `BrokerCore::broadcast_to_server`, `BrokerCore::send_to_leader`
//!    - Tested by: `core::tests::error_handling::session_send_failure_unregisters_session`
//!    - Failure symptom: dead sessions remain registered; leader routing blackholes server-initiated requests.
//!
//! 8. Idle servers MUST be terminated after lease expiry only when no sessions are attached and no inflight requests exist.
//!    - Enforced in: `BrokerCore::check_lease_expiry`
//!    - Tested by: `test_broker_e2e_persistence_lease_expiry`
//!    - Failure symptom: server processes leak indefinitely or are terminated while a request is still in flight.
//!
//! 9. Routing and buffer sync state MUST be protected by independent locks and no path may hold both.
//!    - Enforced in: `BrokerCore::send_event`, `BrokerCore::on_buffer_sync_resync`, `BrokerCore::unregister_session`
//!    - Tested by: `core::tests::lock_partition::test_resync_does_not_require_routing_lock`, `core::tests::lock_partition::test_send_event_does_not_require_sync_lock`
//!    - Failure symptom: routing stalls during resync or deadlocks during session cleanup.
//!
//! 10. On session unregister, broker MUST detach the session from all servers and MUST clean up per-session doc ownership state.
//!     - Enforced in: `BrokerCore::unregister_session`, `BrokerCore::cleanup_session_docs_on_server`
//!     - Tested by: `test_broker_owner_close_transfer`
//!     - Failure symptom: docs remain "owned" by a dead session, blocking updates from remaining sessions and causing stale diagnostics.
//!
//! 11. Diagnostics forwarding MUST prefer the authoritative version from the LSP payload when present, and MAY fall back to broker doc tracking otherwise.
//!     - Enforced in: `LspProxyService::forward`
//!     - Tested by: `core::tests::diagnostics_regression::diagnostics_use_lsp_payload_version_not_broker_version`
//!     - Failure symptom: diagnostics apply to the wrong document version, producing flicker or persistent stale errors.
//!
//! 12. Buffer sync deltas MUST be rejected if the sender is not the owner or epoch/seq do not match.
//!     - Enforced in: `BrokerCore::on_buffer_sync_delta`
//!     - Tested by: `core::tests::buffer_sync::test_buffer_sync_rejects_non_owner`, `core::tests::buffer_sync::test_buffer_sync_seq_mismatch_triggers_resync`
//!     - Failure symptom: follower sessions overwrite the authoritative rope, causing document divergence.
//!
//! 13. Buffer sync deltas MUST validate wire operations and reject malformed edits without mutation.
//!     - Enforced in: `wire_convert::wire_to_tx`, `BrokerCore::on_buffer_sync_delta`
//!     - Tested by: `wire_convert::tests::test_wire_to_tx_rejects_retain_past_eof`, `wire_convert::tests::test_wire_to_tx_rejects_delete_past_eof`, `core::tests::buffer_sync::test_buffer_sync_delta_invalid_tx_is_non_mutating`
//!     - Failure symptom: broker panics or corrupts the authoritative rope/sequence on malformed input.
//!
//! 14. Newly elected owners MUST resync before publishing deltas for the new epoch.
//!     - Enforced in: `BrokerCore::on_buffer_sync_delta`, `BrokerCore::on_buffer_sync_take_ownership`, `BrokerCore::on_buffer_sync_close`, `BrokerCore::cleanup_session_sync_docs`, `BrokerCore::on_buffer_sync_resync`, `BufferSyncManager::handle_owner_changed`, `Editor::update_readonly_for_sync_role`
//!     - Tested by: `core::tests::buffer_sync::test_owner_transfer_requires_resync_before_delta`, `core::tests::buffer_sync::test_owner_disconnect_requires_resync_before_delta`, `core::tests::buffer_sync::test_buffer_sync_take_ownership`, `buffer_sync::tests::test_owner_changed_sets_needs_resync_for_local_new_owner`, `buffer_sync::tests::test_snapshot_clears_needs_resync_and_allows_delta`, `impls::buffer_sync_events::tests::test_new_owner_remains_readonly_until_snapshot`
//!     - Failure symptom: new owner edits diverge from broker state and overwrite follower content.
//!
//! 15. Buffer sync ownership MUST transfer to the minimum remaining [`SessionId`] when the owner disconnects or closes the document.
//!     - Enforced in: `BrokerCore::on_buffer_sync_close`, `BrokerCore::cleanup_session_sync_docs`
//!     - Tested by: `core::tests::buffer_sync::test_buffer_sync_owner_disconnect_elects_successor_epoch_bumps`
//!     - Failure symptom: no session holds ownership after disconnect, blocking all edits until manual resync.
//!
//! 16. Buffer sync epoch MUST increment on every ownership transfer; sequence MUST reset to 0.
//!     - Enforced in: `BrokerCore::on_buffer_sync_take_ownership`, `BrokerCore::on_buffer_sync_close`
//!     - Tested by: `core::tests::buffer_sync::test_buffer_sync_take_ownership`
//!     - Failure symptom: stale-epoch deltas are accepted, applying edits from a previous ownership era.
//!
//! 17. Buffer sync broadcast MUST exclude the sender session and MUST include all other sessions with open refcounts for the URI.
//!     - Enforced in: `BrokerCore::broadcast_to_sync_doc_sessions`
//!     - Tested by: `core::tests::buffer_sync::test_buffer_sync_delta_ack_and_broadcast`
//!     - Failure symptom: sender receives its own delta as a remote edit (infinite loop), or some followers miss deltas.
//!
//! 18. On broker disconnect, the editor MUST clear all buffer sync state and remove buffer sync readonly overrides.
//!     - Enforced in: `Editor::handle_buffer_sync_disconnect`
//!     - Tested by: TODO (add regression: test_buffer_sync_disconnect_clears_readonly)
//!     - Failure symptom: buffers remain stuck in readonly mode after broker disconnect, blocking local editing.
//!
//! 19. KnowledgeCore MUST own its own helix-db instance, separate from the language database.
//!     - Enforced in: `KnowledgeCore::open`
//!     - Tested by: `knowledge::tests::test_knowledge_core_open_close`
//!     - Failure symptom: schema conflicts or data corruption between subsystems.
//!
//! 20. KnowledgeCore MUST degrade gracefully to None if initialization fails.
//!     - Enforced in: `BrokerCore::new_with_config`
//!     - Tested by: `knowledge::tests::test_graceful_degradation`
//!     - Failure symptom: broker crashes on startup if the knowledge DB path is not writable.
//!
//! 21. Knowledge indexing MUST depend on [`knowledge::DocSnapshotSource`] instead of [`BrokerCore`].
//!     - Enforced in: `knowledge::indexer::IndexWorker::spawn`, `knowledge::crawler::ProjectCrawler::spawn`
//!     - Tested by: `knowledge::crawler::tests::test_crawler_skips_open_sync_docs`
//!     - Failure symptom: knowledge indexing cannot be reused or tested without broker coupling.
//!
//! 22. Indexing MUST NOT block buffer sync delta processing.
//!     - Enforced in: `IndexWorker::mark_dirty`, `IndexWorker::spawn`
//!     - Tested by: `knowledge::tests::test_dirty_mark_is_nonblocking`
//!     - Failure symptom: editor input lag during indexing.
//!
//! 23. Epoch/seq validation MUST discard stale indexing results.
//!     - Enforced in: `knowledge::indexer::index_document`
//!     - Tested by: `knowledge::tests::test_stale_index_discarded`
//!     - Failure symptom: search results reference offsets that don't match the current document.
//!
//! 24. Chunk cleanup MUST delete all prior chunks for a URI before re-indexing.
//!     - Enforced in: `knowledge::indexer::index_document`
//!     - Tested by: `knowledge::tests::test_chunk_cleanup_on_reindex`
//!     - Failure symptom: stale chunks cause duplicate or phantom search results.
//!
//! 25. BM25 search MUST filter by label "Chunk" to return content, not metadata.
//!     - Enforced in: `KnowledgeCore::search`
//!     - Tested by: `knowledge::tests::test_search_returns_only_chunks`
//!     - Failure symptom: search returns Doc metadata nodes instead of content.
//!
//! 26. Open-doc reindex MUST preserve existing crawler mtime when `mtime` is None.
//!     - Enforced in: `knowledge::indexer::index_document`
//!     - Tested by: `knowledge::crawler::tests::test_doc_mtime_matches`, `knowledge::crawler::tests::test_index_document_mtime_overridden_when_some`
//!     - Failure symptom: crawler mtime cache resets, forcing full reindex on every open-doc update.
//!
//! 27. Project crawl MUST skip files whose stored mtime matches the on-disk mtime.
//!     - Enforced in: `knowledge::crawler::doc_mtime_matches`, `knowledge::crawler::crawl_project`
//!     - Tested by: `knowledge::crawler::tests::test_doc_mtime_matches`
//!     - Failure symptom: every LSP start reindexes the entire workspace, causing IO spikes.
//!
//! 28. Project crawl MUST skip documents that are currently open in buffer sync.
//!     - Enforced in: `knowledge::crawler::crawl_project`
//!     - Tested by: `knowledge::crawler::tests::test_crawler_skips_open_sync_docs`
//!     - Failure symptom: background crawl overwrites live edits with stale on-disk content.
//!
//! # Data flow
//!
//! ## LSP routing
//!
//! 1. Session connect: Editor connects to broker IPC socket and registers a [`SessionId`] and [`SessionSink`].
//! 2. Server start / attach: Editor requests `LspStart` for a project configuration. Broker deduplicates by [`ProjectKey`]; either starts a new server or attaches to an existing one.
//! 3. Client-to-server messages: Editor sends notifications/requests for `server_id`. Broker rewrites request ids to wire ids and forwards to the LSP server process. Responses are mapped back to the origin session and request id via pending c2s map.
//! 4. Server-to-client messages: LSP server sends: Notifications are broadcast to all attached sessions. Requests are registered as pending s2c and forwarded only to the leader session. Leader session replies; broker completes pending and returns the response to the LSP server.
//! 5. Detach and lease: When the last session detaches, broker schedules lease expiry. If no new sessions attach and no inflight remains at expiry, broker terminates the server.
//!
//! ## Buffer sync
//!
//! 1. Document open: Editor sends `BufferSyncOpen { uri, text }`. First opener becomes Owner with epoch=1, seq=0. Subsequent openers become Followers and receive a snapshot of the current content.
//! 2. Local edit (owner path): Editor applies transaction locally, then calls `BufferSyncManager::prepare_delta` which serializes to `WireTx` and sends `BufferSyncDelta` to the broker. Outbound sender in `LspSystem` awaits the result and posts `DeltaAck` or `DeltaRejected` back to the editor loop.
//! 3. Broker delta processing: Broker validates owner/epoch/seq, converts `WireTx` to `Transaction`, applies to authoritative rope, increments seq, broadcasts `Event::BufferSyncDelta` to all followers, and replies with `BufferSyncDeltaAck { seq }`.
//! 4. Remote delta (follower path): Editor receives `BufferSyncEvent::RemoteDelta`, converts wire tx back to `Transaction`, applies with `UndoPolicy::NoUndo`, and maps selections for all views of the document.
//! 5. Ownership change: On owner disconnect or explicit `TakeOwnership`, broker bumps epoch, resets seq, broadcasts `Event::BufferSyncOwnerChanged`. New owner stays readonly until it resyncs and the broker clears the resync gate; old owner (if still connected) becomes follower (readonly).
//! 6. Document close: Editor sends `BufferSyncClose`. Broker decrements refcount; if owner closed, elects successor (min session ID). Last close removes the entry.
//! 7. Disconnect recovery: On broker transport disconnect, editor calls `BufferSyncManager::disable_all()` and clears buffer sync readonly overrides.
//!
//! ## Knowledge index
//!
//! 1. Startup: Broker initializes [`KnowledgeCore`] on startup; failures are logged and the feature is disabled.
//! 2. Buffer sync events: Document open and delta paths enqueue indexing work for background processing.
//! 3. Debounce: Background worker coalesces updates, snapshots the rope, and reindexes per URI.
//! 4. Project crawl: On LSP start, broker walks the workspace and indexes unopened files, skipping unchanged mtimes.
//! 5. Search: Editor requests return ranked matches from the persistent index.
//!
//! # Lifecycle
//!
//! - Startup: Broker binary starts and initializes [`BrokerCore`] and IPC loop.
//! - Session registration: Each editor session registers with a [`SessionId`] and sink.
//! - Server registration: Broker starts or reuses an LSP server instance, assigns [`ServerId`], attaches session, elects leader.
//! - Running: Broker proxies JSON-RPC in both directions and maintains pending request maps. Buffer sync deltas are validated and applied to the authoritative rope.
//! - Leader change: Detach of the leader triggers re-election to min(attached) and cancels pending s2c for the old leader.
//! - Buffer sync open: Editor calls `BufferSyncOpen` during buffer lifecycle; broker creates or joins a [`SyncDocState`].
//! - Buffer sync ownership transfer: On owner disconnect or explicit request, broker bumps epoch, broadcasts `OwnerChanged`, and the new owner must resync before publishing deltas.
//! - Buffer sync close: Editor calls `BufferSyncClose` during buffer removal; broker decrements refcount and elects successor if needed.
//! - Idle lease: When attached is empty, broker schedules lease expiry; server remains warm until expiry conditions are met.
//! - Session cleanup: `cleanup_session_sync_docs` removes the disconnected session from all sync docs and transfers ownership as needed.
//! - Termination: On lease expiry with no inflight, or on explicit termination, broker stops the server and removes indices.
//! - Shutdown: Broker terminates all servers and clears state.
//!
//! # Concurrency and ordering
//!
//! - BrokerCore state access: routing and sync state are protected by separate locks; code MUST NOT hold both, and MUST release locks before IPC sends or `rope.to_string()`.
//! - Pending request ordering: Server-to-client requests are routed to leader and completed by matching request id. Client implementations MUST preserve FIFO request/reply pairing if they use a queue-based strategy.
//! - Background tasks: Lease expiry runs in a spawned task and MUST re-check generation tokens to avoid stale termination. Server monitor tasks MUST report exits and trigger cleanup.
//! - Knowledge indexing: Indexing work runs in a background task and MUST snapshot under the sync lock, then release before `rope.to_string()` and LMDB writes.
//! - Knowledge crawling: Project crawl runs in a blocking task and MUST only briefly query `DocSnapshotSource::is_sync_doc_open` to skip open docs.
//!
//! # Failure modes and recovery
//!
//! - Session IPC disconnect: Broker detects send failure and unregisters session; pending requests for that session are cancelled; buffer sync docs owned by this session transfer ownership.
//! - Leader disconnect: Broker cancels pending s2c requests for the old leader and elects a new leader if possible.
//! - Server crash: Broker marks server stopped, cancels inflight, and removes server indices; subsequent start attaches to a fresh server.
//! - Request timeout (server-to-client): Broker cancels pending and replies with `REQUEST_CANCELLED` error to the server.
//! - Dedup mismatch: If [`ProjectKey`] construction is wrong, broker shares servers incorrectly; fix `ProjectKey` normalization and add regression tests.
//! - Buffer sync epoch mismatch: Follower delta rejected with `SyncEpochMismatch`; editor should request resync.
//! - Buffer sync seq mismatch: Delta rejected with `SyncSeqMismatch`; editor should request resync to recover.
//! - Buffer sync invalid delta: Delta rejected with `InvalidDelta`; editor should request resync and retry after snapshot.
//! - Buffer sync owner resync gate: New owner delta rejected with `OwnerNeedsResync`; editor should resync before sending edits.
//! - Broker disconnect (editor side): Editor clears all sync state via `disable_all()` and removes readonly overrides so local editing resumes.
//! - Knowledge DB unavailable: Broker logs a warning and returns `NotImplemented` for knowledge queries.
//!
//! # Recipes
//!
//! ## Add a new broker IPC event
//!
//! - Extend `xeno_broker_proto::types::Event`.
//! - Update broker broadcast/send sites to emit the new event.
//! - Update the editor transport event mapping to surface it as a `TransportEvent` or UI event.
//!
//! ## Debug broker routing issues with file logs
//!
//! - Set `XENO_LOG_DIR` and `RUST_LOG` in the editor environment.
//! - Ensure editor spawns broker with env propagation.
//! - Inspect `xeno-broker.<pid>.log` for: attach/detach and leader re-election logs, pending map registration/completion/cancellation, lease scheduling and termination decisions.
//!
//! ## Verify multi-process dedup
//!
//! - Run two editors against the same workspace concurrently.
//! - Confirm broker spawns one server process and attaches both sessions to the same [`ServerId`].
//!
//! ## Verify buffer sync two-terminal
//!
//! - Open the same file in two terminal windows.
//! - Type in one window; confirm the other receives the edit in real-time.
//! - Close the owner terminal; confirm the follower terminal becomes the new owner and can edit.
//!
//! ## Add a new buffer sync event
//!
//! - Add the variant to `BufferSyncEvent` in `editor::buffer_sync::mod`.
//! - Handle it in `BrokerClientService::notify()` in `editor::lsp::broker_transport`.
//! - Dispatch it in `Editor::handle_buffer_sync_event()` in `editor::impls::buffer_sync_events`.

mod buffer_sync;
mod events;
mod knowledge;
mod routing;
mod server;
mod session;
mod text_sync;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ropey::Rope;
// Re-export public types from submodules
pub use server::{ChildHandle, LspInstance, ServerControl};
pub use text_sync::{DocGateDecision, DocGateKind, DocGateResult};
use tokio::sync::oneshot;
use xeno_broker_proto::types::{
	ErrorCode, IpcFrame, LspServerConfig, Request, Response, ResponsePayload, ServerId, SessionId,
	SyncEpoch, SyncSeq,
};
use xeno_rpc::PeerSocket;

/// Sink for sending events to a connected session.
pub type SessionSink = PeerSocket<IpcFrame, Request, Response>;

/// Sink for sending messages to an LSP server.
pub type LspTx = PeerSocket<xeno_lsp::Message, xeno_lsp::AnyRequest, xeno_lsp::AnyResponse>;

/// Result of a server-to-editor request.
pub type LspReplyResult = Result<serde_json::Value, xeno_lsp::ResponseError>;

/// Configuration for the broker core.
#[derive(Debug, Clone)]
pub struct BrokerConfig {
	/// Duration to keep an idle server alive after all sessions detach.
	pub idle_lease: Duration,
}

impl Default for BrokerConfig {
	fn default() -> Self {
		Self {
			idle_lease: Duration::from_secs(300), // 5 minutes default
		}
	}
}

/// Shared state for the broker.
///
/// Routing and buffer sync state are protected by independent locks
/// to avoid contention between IPC routing and document synchronization.
#[derive(Debug)]
pub struct BrokerCore {
	routing: Mutex<RoutingState>,
	sync: Mutex<SyncState>,
	next_server_id: AtomicU64,
	config: BrokerConfig,
	knowledge: Option<Arc<knowledge::KnowledgeCore>>,
}

impl Default for BrokerCore {
	fn default() -> Self {
		Self {
			routing: Mutex::new(RoutingState::default()),
			sync: Mutex::new(SyncState::default()),
			next_server_id: AtomicU64::new(0),
			config: BrokerConfig::default(),
			knowledge: None,
		}
	}
}

#[derive(Debug, Default)]
struct RoutingState {
	sessions: HashMap<SessionId, SessionEntry>,
	servers: HashMap<ServerId, ServerEntry>,
	projects: HashMap<ProjectKey, ServerId>,
	/// Pending server-to-client requests awaiting an editor reply.
	///
	/// These are routed only to the server leader.
	pending_s2c: HashMap<(ServerId, xeno_lsp::RequestId), PendingS2cReq>,
	/// Pending client-to-server requests awaiting an LSP server response.
	///
	/// The broker rewrites these IDs to prevent collisions between sessions.
	pending_c2s: HashMap<(ServerId, xeno_lsp::RequestId), PendingC2sReq>,
}

#[derive(Debug, Default)]
struct SyncState {
	/// Buffer sync documents keyed by canonical URI.
	sync_docs: HashMap<String, SyncDocState>,
}

/// Broker-authoritative state for a single buffer sync document.
///
/// Tracks ownership, refcounts, sequence ordering, resync gating, and the
/// authoritative rope content for a synchronized document.
#[derive(Debug)]
struct SyncDocState {
	/// Current owner session (single-writer).
	owner: SessionId,
	/// Per-session open refcounts.
	open_refcounts: HashMap<SessionId, u32>,
	/// Current ownership epoch (monotonically increasing).
	epoch: SyncEpoch,
	/// Current edit sequence number within the epoch.
	seq: SyncSeq,
	/// Authoritative document content.
	rope: Rope,
	/// Newly elected owners must resync before publishing deltas.
	owner_needs_resync: bool,
}

/// Unique key for deduplicating LSP servers by project identity.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ProjectKey {
	/// Command used to start the server.
	pub command: String,
	/// Arguments passed to the command.
	pub args: Vec<String>,
	/// Project root directory.
	pub cwd: String,
}

impl From<&LspServerConfig> for ProjectKey {
	fn from(cfg: &LspServerConfig) -> Self {
		let cwd = cfg.cwd.clone().unwrap_or_else(|| {
			// Configs without cwd should not dedup with each other unless
			// command and args match exactly.
			format!("__NO_CWD_{:016x}__", compute_config_hash(cfg))
		});
		Self {
			command: cfg.command.clone(),
			args: cfg.args.clone(),
			cwd,
		}
	}
}

/// Compute a stable hash of config for the no-cwd sentinel.
fn compute_config_hash(cfg: &LspServerConfig) -> u64 {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};
	let mut hasher = DefaultHasher::new();
	cfg.command.hash(&mut hasher);
	cfg.args.hash(&mut hasher);
	hasher.finish()
}

#[derive(Debug)]
struct SessionEntry {
	sink: SessionSink,
	attached: HashSet<ServerId>,
}

/// State for a single LSP server instance managed by the broker.
#[derive(Debug)]
struct ServerEntry {
	/// Handle to the running LSP process and its control channels.
	instance: LspInstance,
	/// Identity of the server used for deduplication.
	project: ProjectKey,
	/// Editor sessions currently attached to this server.
	attached: HashSet<SessionId>,
	/// Session responsible for answering server-initiated requests.
	leader: SessionId,
	/// Tracked documents and their versions for this server.
	docs: text_sync::DocRegistry,
	/// Generation token used to invalidate stale lease tasks.
	lease_gen: u64,
	/// Ownership registry for text sync gating (single-writer per URI).
	doc_owners: text_sync::DocOwnerRegistry,
	/// Monotonic allocator for broker "wire ids" for requests sent to the LSP server.
	next_wire_req_id: u64,
}

/// Metadata for a pending server-to-client request awaiting a reply from the editor.
#[derive(Debug)]
struct PendingS2cReq {
	/// Session elected as leader at the time of the request.
	responder: SessionId,
	/// Completion channel for the proxied LSP response.
	tx: oneshot::Sender<LspReplyResult>,
}

/// Metadata for a pending client-to-server request awaiting a response from the LSP server.
#[derive(Debug)]
pub struct PendingC2sReq {
	/// Editor session that initiated the request.
	pub origin_session: SessionId,
	/// Original request id as seen by the editor.
	pub origin_id: xeno_lsp::RequestId,
}

/// Snapshot of the current broker state for debugging or testing.
pub type BrokerStateSnapshot = (
	HashSet<SessionId>,
	HashMap<ServerId, Vec<SessionId>>,
	HashMap<ProjectKey, ServerId>,
);

impl BrokerCore {
	/// Create a new broker core instance with default configuration.
	#[must_use]
	pub fn new() -> Arc<Self> {
		Self::new_with_config(BrokerConfig::default())
	}

	/// Create a new broker core instance with custom configuration.
	#[must_use]
	pub fn new_with_config(config: BrokerConfig) -> Arc<Self> {
		let knowledge = match knowledge::default_db_path()
			.and_then(|path| knowledge::KnowledgeCore::open(path).map(Arc::new))
		{
			Ok(core) => Some(core),
			Err(err) => {
				tracing::warn!(error = %err, "KnowledgeCore disabled");
				None
			}
		};

		let core = Arc::new(Self {
			routing: Mutex::new(RoutingState::default()),
			sync: Mutex::new(SyncState::default()),
			next_server_id: AtomicU64::new(0),
			config,
			knowledge,
		});

		if let Some(knowledge) = &core.knowledge {
			let source: Arc<dyn knowledge::DocSnapshotSource> = core.clone();
			knowledge.start_worker(Arc::downgrade(&source));
		}

		core
	}

	/// Retrieves a snapshot of the current broker state for debugging or testing.
	#[doc(hidden)]
	pub fn get_state(&self) -> BrokerStateSnapshot {
		let routing = self.routing.lock().unwrap();
		let sessions = routing.sessions.keys().cloned().collect();
		let servers = routing
			.servers
			.iter()
			.map(|(id, s)| (*id, s.attached.iter().cloned().collect()))
			.collect();
		let projects = routing.projects.clone();
		(sessions, servers, projects)
	}

	/// Executes a knowledge search query against the persistent index.
	pub fn knowledge_search(&self, query: &str, limit: u32) -> Result<ResponsePayload, ErrorCode> {
		let Some(knowledge) = &self.knowledge else {
			return Err(ErrorCode::NotImplemented);
		};

		match knowledge.search(query, limit) {
			Ok(hits) => Ok(ResponsePayload::KnowledgeSearchResults { hits }),
			Err(err) => {
				tracing::warn!(error = %err, "knowledge search failed");
				Err(ErrorCode::Internal)
			}
		}
	}

	/// Retrieves the communication handle for a specific LSP server.
	pub fn get_server_tx(&self, server_id: ServerId) -> Option<LspTx> {
		let routing = self.routing.lock().unwrap();
		routing
			.servers
			.get(&server_id)
			.map(|s| s.instance.lsp_tx.clone())
	}

	/// Spawns a background project crawl for the server's workspace, if configured.
	pub fn spawn_project_crawl(self: &Arc<Self>, config: &LspServerConfig) {
		let Some(knowledge) = self.knowledge.clone() else {
			return;
		};
		let Some(cwd) = config.cwd.as_ref() else {
			return;
		};

		let root = PathBuf::from(cwd);
		let source: Arc<dyn knowledge::DocSnapshotSource> = self.clone();
		let _ = knowledge::crawler::ProjectCrawler::spawn(knowledge, Arc::downgrade(&source), root);
	}

	/// Allocate a globally unique server ID.
	pub fn next_server_id(&self) -> ServerId {
		ServerId(self.next_server_id.fetch_add(1, Ordering::Relaxed))
	}
}

impl knowledge::DocSnapshotSource for BrokerCore {
	fn snapshot_sync_doc(&self, uri: &str) -> Option<(SyncEpoch, SyncSeq, Rope)> {
		let sync = self.sync.lock().unwrap();
		let doc = sync.sync_docs.get(uri)?;
		Some((doc.epoch, doc.seq, doc.rope.clone()))
	}

	fn is_sync_doc_open(&self, uri: &str) -> bool {
		let sync = self.sync.lock().unwrap();
		sync.sync_docs.contains_key(uri)
	}
}

#[cfg(test)]
impl BrokerCore {
	fn lock_routing_for_test(&self) -> std::sync::MutexGuard<'_, RoutingState> {
		self.routing.lock().unwrap()
	}

	fn lock_sync_for_test(&self) -> std::sync::MutexGuard<'_, SyncState> {
		self.sync.lock().unwrap()
	}
}

#[cfg(test)]
mod tests;
