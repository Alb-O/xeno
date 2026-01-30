# LSP System Architecture

## Purpose
- Owns: LSP client stack, JSON-RPC transport, document sync policy, and feature-specific controllers.
- Does not own: Document content (owned by `Document`), UI presentation (delegated via events).
- Source of truth: `LspSystem` in `crates/editor/src/lsp/system.rs` (wraps `xeno_lsp::LspManager`).
- Shared Sessions: Managed by a central broker daemon to deduplicate LSP servers across multiple editor windows.

## Mental model
- Terms: Client (server instance), Sync (didOpen/didChange), Generation (staleness token), Outbox (single writer), Broker (shared manager), ProjectKey (deduplication token), Lease (persistence window), Leader (request responder).
- Lifecycle in one sentence: Sessions attach to project-scoped LSP servers via a broker; servers remain warm under an idle lease and fan-out notifications to all attached clients.

## Module map
- `crates/lsp/` — Core framework, transport, and protocol implementation.
- `crates/broker/` — Broker daemon, project deduplication, and lease management.
- `crates/editor/src/lsp/system.rs` — Integration root and public API.
- `crates/editor/src/lsp/broker_transport.rs` — IPC bridge between editor and broker.
- `crates/editor/src/lsp/sync_manager/` — Coalescing and sync logic.
- `crates/editor/src/lsp/events.rs` — UI event fanout and state application.

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| `ClientHandle` | Communication channel | MUST check `Ready` state | `crates/editor/src/lsp/system.rs`::`LspSystem::on_buffer_open` |
| `LspUiEvent` | Async UI result | Validated by generation | `crates/editor/src/lsp/events.rs`::`handle_lsp_ui_event` |
| `DocSyncState` | Server's view of doc | MUST only apply changes in-order | `crates/editor/src/lsp/sync_manager/mod.rs`::`LspSyncManager` |
| `ProjectKey` | Server identity | Deduplicated by cmd/args/cwd | `crates/broker/broker/src/core/mod.rs`::`ProjectKey` |
| `ServerControl` | Lifecycle channels | Monitor task owns Child; LspInstance uses control channel | `crates/broker/broker/src/launcher.rs` (spawn) / `crates/broker/broker/src/core/mod.rs`::`LspInstance` |

## Invariants (hard rules)
1. MUST canonicalize all paths before LSP calls.
   - Enforced in: `crates/editor/src/lsp/system.rs`::`LspSystem::canonicalize_path`
   - Tested by: TODO (add regression: test_path_normalization)
   - Failure symptom: Document identity desync (multiple entries for same file).
2. MUST NOT process UI events if modal overlay is open.
   - Enforced in: `crates/editor/src/lsp/events.rs`::`Editor::handle_lsp_ui_event`
   - Tested by: TODO (add regression: test_modal_gating_lsp_ui)
   - Failure symptom: Completion menus appearing on top of command palette.
3. MUST NOT send requests/notifications until client is in Ready.
   - Enforced in: `crates/lsp/src/client/lifecycle.rs`::`init_handshake`
   - Tested by: TODO (add regression: test_ready_gate)
   - Failure symptom: Race condition where server ignores early notifications.
4. MUST terminate idle servers after lease expiry.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`BrokerCore::check_lease_expiry`
   - Tested by: `crates/broker/broker/src/core/tests.rs`::`test_lease_expiry_terminates_server`
   - Failure symptom: LSP processes leak after all editor sessions are closed.
5. MUST register pending server→client requests before forwarding to prevent race.
   - Enforced in: `crates/broker/broker/src/lsp.rs`::`LspProxyService::call`
   - Tested by: `crates/broker/broker/src/core/tests.rs`::`reply_from_leader_completes_pending`
   - Failure symptom: Server→client replies arrive before broker registers pending, causing "request not found" errors.
6. MUST cancel pending requests on timeout, leader detach, and server exit.
   - Enforced in: `crates/broker/broker/src/lsp.rs`::`LspProxyService::call` (timeout), `crates/broker/broker/src/core/mod.rs`::`cancel_client_request` (detach/exit)
   - Tested by: `crates/broker/broker/src/core/tests.rs`::`disconnect_leader_cancels_pending_requests`
   - Failure symptom: Memory leaks in pending_client_reqs map, or late replies misdelivered to wrong sessions.
7. MUST detect and handle server process exit (crash or graceful).
   - Enforced in: `crates/broker/broker/src/launcher.rs` (spawn monitor), `crates/broker/broker/src/core/mod.rs`::`handle_server_exit`
   - Tested by: `crates/broker/broker/src/core/tests.rs` (server lifecycle tests)
   - Failure symptom: Crashed servers remain "Running" indefinitely; new sessions attach to dead processes.
8. MUST trigger session cleanup on send failure.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`broadcast_to_server`, `send_to_leader`, `set_server_status`
   - Tested by: Implicit in disconnect tests
   - Failure symptom: Dead sessions remain registered; leader routing blackholes requests.
9. MUST deduplicate projects by normalized ProjectKey (no empty cwd).
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`ProjectKey::from`
   - Tested by: `crates/broker/broker/src/core/tests.rs`::`project_dedup_*` tests
   - Failure symptom: Unrelated projects incorrectly share LSP servers, causing document identity confusion.

## Data flow
1. Trigger: User types or triggers manual completion.
2. Request: `LspSystem` builds request; `BrokerTransport` forwards to broker over Unix socket.
3. Proxy: Broker routes request to the specific LSP server instance.
4. Async boundary: LSP server sends JSON-RPC; broker awaits response and routes back to correct session.
5. Server→Client requests: Broker registers pending request BEFORE forwarding to leader. Reply completes pending; timeout/leader-change/server-exit cancels with REQUEST_CANCELLED error.
6. Fan-out: Server notifications (diagnostics/status) are broadcast by broker to all attached sessions.
7. Validation: Result matches current generation and buffer version?
8. Effect: Apply edits or open menu via `LspUiEvent`.

## Lifecycle
- Starting: Server process spawning and initializing.
- Ready: Handshake complete; accepting edits/requests.
- Leased: No sessions attached; server remains warm for reattachment.
- Dead: Process crashed or lease expired; broker cleans up.

## Concurrency & ordering
- Single-writer Outbox: Only one task writes to the socket.
- Monotonic Generations: Every completion/signature request has a unique incrementing ID.
- Leader Election: Server->Client requests are routed only to the first-attached "leader" session.

## Failure modes & recovery
- Server Crash: Mark client `Dead` and notify all sessions; broker cleans up.
- Lease Expiry: Server terminated gracefully; next session will spawn a fresh instance.
- Broker Disconnect: Editor falls back to `Dead` state for all brokered servers.
- Canonicalization Failure: Skip LSP operations for that buffer to avoid identity aliasing.

## Recipes
### Add a new LSP feature
Steps:
- Add typed method to `xeno_lsp::client::api`.
- Implement prepare/send logic in `LspSystem`.
- Handle result/notification in `LspEventHandler` or `drain_lsp_ui_events`.

## Tests
- `crates/broker/broker/src/core/tests.rs`::`test_lease_expiry_terminates_server`
- `crates/broker/broker/src/core/tests.rs`::`test_warm_reattach_reuses_server`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_dedup_and_fanout`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_leader_routing_and_reply`
- `crates/editor/src/lsp/sync_manager/tests.rs`::`test_doc_open_close`
- `crates/editor/src/lsp/sync_manager/tests.rs`::`test_contiguity_check_success`

## Glossary
- Client: A session's handle to a Language Server (local or brokered).
- Broker: Daemon managing shared LSP server instances.
- ProjectKey: Unique identifier for a project root and server configuration.
- Lease: Configurable time window where an idle server is kept warm.
- Leader: The session responsible for answering server-initiated requests.
- Sync: The protocol for keeping the server's view of a document in sync with the editor.
- Generation: A unique identifier for a specific UI request to prevent applying stale results.
- Outbox: The bounded queue of outgoing LSP messages.
