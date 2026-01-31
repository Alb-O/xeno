# LSP

## Purpose
- Define the editor-side LSP client stack: document synchronization, server registry, transport integration, and server-initiated request handling.
- Describe the broker-default transport behavior used by the editor on Unix.
- Exclude broker daemon internals (deduplication, leases, leader routing, pending maps); see docs/agents/broker.md.

## Mental model
- LspSystem is the editor integration root that constructs an LspManager with a broker-backed transport.
- DocumentSync owns didOpen/didChange/didSave/didClose policy and the local DocumentStateManager (diagnostics, progress).
- Registry maps (language, workspace root) to a ClientHandle and enforces singleflight for server startup.
- LspManager::spawn_router is the event pump that applies TransportEvent streams to DocumentStateManager and replies to server-initiated requests in-order.
- BrokerTransport is the only production transport on Unix builds; it carries JSON-RPC frames over an IPC channel to the broker daemon.

## Module map
- `crates/editor/src/lsp/system.rs` — Editor integration root that constructs LspManager and exposes LspHandle.
- `crates/editor/src/lsp/broker_transport.rs` — Broker-backed implementation of xeno_lsp::client::transport::LspTransport. IPC connection caching and disconnect invalidation.
- `crates/lsp/src/sync/mod.rs` — DocumentSync and DocumentSyncEventHandler. didOpen/didChange/didSave/didClose policy and barrier semantics.
- `crates/lsp/src/registry.rs` — Registry and singleflight server startup. Server metadata used for server-initiated request handlers.
- `crates/lsp/src/session/manager.rs` — LspManager and the router task (TransportEvent pump).
- `crates/lsp/src/session/server_requests.rs` — Server-initiated request handlers (workspace/configuration, workspaceFolders, etc.).
- `crates/term/src/main.rs` — File-based tracing setup (xeno.<pid>.log) and headless lsp-smoke command.

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| LspSystem | Editor integration root for LSP | MUST construct an LspManager with a broker transport on Unix | `crates/editor/src/lsp/system.rs`::`LspSystem::new` |
| LspManager | Owns DocumentSync and routes transport events | MUST reply to server-initiated requests inline to preserve request/reply pairing | `crates/lsp/src/session/manager.rs`::`LspManager::spawn_router` |
| DocumentSync | High-level doc sync coordinator | MUST gate change notifications on client initialization state | `crates/lsp/src/sync/mod.rs`::`DocumentSync::*` |
| Registry | Maps (language, root_path) to a running client | MUST singleflight transport.start() per key | `crates/lsp/src/registry.rs`::`Registry::get_or_start` |
| RegistryState | Consolidated registry indices | MUST update servers/server_meta/id_index atomically | `crates/lsp/src/registry.rs`::`Registry::get_or_start`, `Registry::remove_server` |
| ServerMeta | Per-server metadata for server-initiated requests | MUST be removable by server id | `crates/lsp/src/registry.rs`::`Registry::get_or_start`, `Registry::remove_server` |
| ClientHandle | RPC handle for a single language server instance | MUST NOT be treated as ready until initialization completes | `crates/lsp/src/client/handle.rs`::`ClientHandle::*` |
| TransportEvent | Transport → manager event stream | Router MUST process sequentially | `crates/lsp/src/session/manager.rs`::`LspManager::spawn_router` |
| TransportStatus | Lifecycle signals for server processes | Router MUST remove servers on Stopped/Crashed | `crates/lsp/src/session/manager.rs`::`LspManager::spawn_router` |
| BrokerTransport | Broker-backed LSP transport | MUST invalidate cached state on send failure | `crates/editor/src/lsp/broker_transport.rs`::`BrokerTransport::mark_disconnected` |

## Invariants (hard rules)
1. The editor MUST use the broker transport on Unix builds.
   - Enforced in: `crates/editor/src/lsp/system.rs`::`LspSystem::new`
   - Tested by: TODO (add regression: test_lsp_system_uses_broker_transport_on_unix)
   - Failure symptom: LSP requests silently do nothing or attempt to use removed LocalTransport code paths.
2. Registry startup MUST singleflight transport.start() per (language, root_path) key.
   - Enforced in: `crates/lsp/src/registry.rs`::`Registry::get_or_start`
   - Tested by: TODO (add regression: test_registry_singleflight_prevents_duplicate_transport_start)
   - Failure symptom: duplicate broker LspStart calls, leaked server processes until lease expiry, inconsistent server ids across callers.
3. Registry mutations MUST be atomic across servers, server_meta, and id_index.
   - Enforced in: `crates/lsp/src/registry.rs`::`Registry::get_or_start`, `Registry::remove_server`
   - Tested by: TODO (add regression: test_registry_remove_server_scrubs_all_indices)
   - Failure symptom: stale server metadata persists after removal, status cleanup fails to fully detach, server request handlers read wrong settings/root.
4. The router MUST process transport events sequentially and MUST reply to server-initiated requests inline.
   - Enforced in: `crates/lsp/src/session/manager.rs`::`LspManager::spawn_router`
   - Tested by: `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_leader_routing_and_reply`
   - Failure symptom: server request/reply pairing breaks, replies go to the wrong pending request, server-side hangs waiting for a response.
5. On TransportStatus::Stopped or TransportStatus::Crashed, the router MUST remove the server from Registry and MUST clear per-server progress.
   - Enforced in: `crates/lsp/src/session/manager.rs`::`LspManager::spawn_router`, `crates/lsp/src/registry.rs`::`Registry::remove_server`
   - Tested by: TODO (add regression: test_status_stopped_removes_server_and_clears_progress)
   - Failure symptom: UI shows stuck progress forever, stale ClientHandle remains reachable, subsequent requests wedge on a dead server id.
6. workspace/configuration handling MUST return an array with one element per requested item, and MUST return an object for missing config.
   - Enforced in: `crates/lsp/src/session/server_requests.rs`::`handle_workspace_configuration`
   - Tested by: TODO (add regression: test_server_request_workspace_configuration_section_slicing)
   - Failure symptom: servers treat configuration as invalid, disable features, or log repeated configuration query errors.
7. workspace/workspaceFolders handling MUST return percent-encoded file URIs.
   - Enforced in: `crates/lsp/src/session/server_requests.rs`::`handle_workspace_folders`
   - Tested by: TODO (add regression: test_server_request_workspace_folders_uri_encoding)
   - Failure symptom: servers mis-parse the workspace root for paths with spaces or non-ASCII characters and degrade indexing/navigation.
8. BrokerTransport MUST invalidate cached RPC state and per-server request queues on send failure.
   - Enforced in: `crates/editor/src/lsp/broker_transport.rs`::`BrokerTransport::mark_disconnected`
   - Tested by: `crates/editor/tests/broker_e2e.rs`::`test_broker_reconnect_wedge`
   - Failure symptom: reconnect wedges (stale cached RPC), servers never expire, or pending request queues grow unbounded.
9. DocumentSync MUST NOT send change notifications before the client has completed initialization.
   - Enforced in: `crates/lsp/src/sync/mod.rs`::`DocumentSync::notify_change_full_text`, `DocumentSync::notify_change_incremental_no_content`
   - Tested by: TODO (add regression: test_document_sync_returns_not_ready_before_init)
   - Failure symptom: edits are dropped by the server or applied out of order, resulting in stale diagnostics and incorrect completions.

## Data flow
1. Editor constructs LspSystem which constructs LspManager with BrokerTransport.
2. Editor opens a buffer; DocumentSync chooses a language and calls Registry::get_or_start(language, path).
3. Registry singleflights startup and obtains a ClientHandle for the (language, root_path) key.
4. DocumentSync registers the document in DocumentStateManager and sends didOpen via ClientHandle.
5. Subsequent edits call DocumentSync change APIs; DocumentStateManager assigns versions; change notifications are sent and acknowledged.
6. BrokerTransport forwards JSON-RPC frames to the broker daemon over Unix domain socket IPC.
7. Transport emits TransportEvent values; LspManager router consumes them:
   - Diagnostics events update DocumentStateManager diagnostics.
   - Message events: Requests are handled by handle_server_request and replied via transport.reply. Notifications update progress and may be logged.
   - Status events remove crashed/stopped servers from Registry and clear progress.
   - Disconnected events stop the router loop.

## Lifecycle
- Configuration: Editor registers LanguageServerConfig via LspManager::configure_server.
- Startup: First open/change triggers Registry::get_or_start and transport start. Client initialization runs asynchronously; readiness is tracked by ClientHandle.
- Running: didOpen/didChange/didSave/didClose flow through DocumentSync. Router updates diagnostics/progress and services server-initiated requests.
- Stopped/Crashed: Transport emits status; router removes server from Registry and clears progress. Next operation will start a new server instance.
- Disconnected: BrokerTransport invalidates cached state; router exits on Disconnected. Next operation triggers reconnect and restart as needed.

## Concurrency & ordering
- Registry startup ordering: Registry MUST ensure only one transport.start() runs for a given (language, root_path) key at a time. Waiters MUST block on the inflight gate and then re-check the RegistryState.
- Router ordering: LspManager router MUST process events in the order received from the transport receiver. Server-initiated requests MUST be handled inline; do not spawn per-request tasks that reorder replies.
- Document versioning: DocumentStateManager versions MUST be monotonic per URI. When barriers are used, DocumentSync MUST only ack_change after the barrier is resolved.

## Failure modes & recovery
- Duplicate startup attempt: Recovery: singleflight blocks duplicates; waiters reuse the leader's handle.
- Broker IPC send failure: Recovery: BrokerTransport calls mark_disconnected; subsequent operation reconnects.
- Server crash or stop: Recovery: router removes server; subsequent operation re-starts server via Registry.
- Unsupported server-initiated request method: Recovery: handler returns METHOD_NOT_FOUND; add method to allowlist if required by real servers.
- URI conversion failure for workspaceFolders: Recovery: handler returns empty array; server may operate without workspace folders.

## Recipes
### Add a new server-initiated request handler
- Implement a method arm in `crates/lsp/src/session/server_requests.rs`.
- Return a stable, schema-valid JSON value for the LSP method.
- Ensure the handler is called inline from LspManager::spawn_router.
- Add a regression test: TODO (add regression: test_server_request_<method_name>).

### Add a new LSP feature request from the editor
- Add a typed API method on ClientHandle or a controller in crates/lsp.
- Call through DocumentSync or a feature controller from editor code.
- Gate on readiness and buffer identity invariants (URI, version).
- Plumb results into editor UI through the existing event mechanism.

### Run headless smoke verification with file-based tracing
- Set XENO_LOG_DIR and RUST_LOG.
- Run `xeno lsp-smoke <workspace_path>`.
- Grep for: singleflight leader path (exactly one after transport.start), server-initiated request handling logs, status updates and disconnects.

### Enable broker + editor log correlation
- Ensure broker spawn passes XENO_LOG_DIR, RUST_LOG, XENO_LOG, RUST_BACKTRACE.
- Correlate by PID filenames: xeno.<pid>.log and xeno-broker.<pid>.log.

## Tests
- `crates/lsp/src/session/manager.rs`::`tests::test_lsp_manager_creation`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_reconnect_wedge`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_leader_routing_and_reply`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_dedup_and_fanout`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_persistence_warm_reattach`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_persistence_lease_expiry`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_owner_close_transfer`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_string_wire_ids`
- `crates/editor/src/lsp/sync_manager/tests.rs`::`test_doc_open_close`
- `crates/editor/src/lsp/sync_manager/tests.rs`::`test_contiguity_check_success`
- TODO (add regression: test_registry_singleflight_prevents_duplicate_transport_start)
- TODO (add regression: test_status_stopped_removes_server_and_clears_progress)
- TODO (add regression: test_server_request_workspace_configuration_section_slicing)
- TODO (add regression: test_server_request_workspace_folders_uri_encoding)

## Glossary
- broker transport: The LspTransport implementation in crates/editor/src/lsp/broker_transport.rs that forwards JSON-RPC over IPC to the broker daemon.
- client handle: A ClientHandle representing one running language server instance for a (language, root_path) key.
- disconnect invalidation: The act of clearing cached IPC state and pending queues so the next operation forces reconnect.
- inflight gate: Registry singleflight mechanism that ensures only one startup attempt per (language, root_path) key.
- registry: The mapping layer from language and file path to a running client instance.
- server-initiated request: A JSON-RPC request sent from the language server to the client that requires a reply (handled by server_requests.rs).
- status event: A TransportEvent::Status emitted by the transport to report server lifecycle (Starting/Running/Stopped/Crashed).
- workspace root: The root directory selected by Registry from root markers and file path; used for server identity and workspaceFolders replies.
