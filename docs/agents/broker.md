# Broker

## Purpose
- Define the broker daemon that deduplicates, shares, and supervises language server processes across editor sessions.
- Describe broker-side routing rules for server↔client JSON-RPC, including leader election, pending request tracking, and lease-based persistence.
- Exclude editor-side document sync and UI integration; see docs/agents/lsp.md.

## Mental model
- The broker is an out-of-process daemon that owns the actual LSP server processes.
- Editor sessions connect to the broker via IPC and register a SessionId.
- Each LSP server instance is keyed by ProjectKey and shared across sessions that attach to that server.
- Server→client requests are routed only to the leader session (deterministic: minimum SessionId).
- Client→server requests are rewritten to broker-allocated wire request ids to avoid collisions between sessions.
- The broker keeps idle servers alive for an idle lease duration; after lease expiry and with no inflight requests, the server is terminated.

## Module map
- `crates/broker/broker/src/core/mod.rs` — BrokerCore state machine: sessions, servers, project deduplication, leader election, pending request maps, leases.
- `crates/broker/broker/src/lsp.rs` — LspProxyService: proxy between an LSP server stdio connection and BrokerCore event routing.
- `crates/broker/broker/src/launcher.rs` — Spawns and monitors LSP server child processes and reports exit to core.
- `crates/broker/broker-bin/src/main.rs` — Broker binary entrypoint and file-based tracing setup (xeno-broker.<pid>.log).
- `crates/editor/src/lsp/broker_transport.rs` — Broker daemon spawn logic and environment propagation from editor to broker.
- `crates/broker_proto/*` — IPC frame and event types: IpcFrame, Event, Request, Response, LspServerConfig, ServerId, SessionId.

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| BrokerCore | Authoritative broker state machine | MUST be the only owner of session/server maps | `crates/broker/broker/src/core/mod.rs`::`BrokerCore::*` |
| ProjectKey | Dedup key for LSP servers | MUST uniquely represent command/args/cwd (with no-cwd sentinel) | `crates/broker/broker/src/core/mod.rs`::`ProjectKey::from` |
| ServerEntry | One managed LSP server instance | MUST maintain leader = min(attached) | `crates/broker/broker/src/core/mod.rs`::`BrokerCore::attach_session`, `BrokerCore::detach_session` |
| SessionEntry | One connected editor session | MUST track attachment set for cleanup | `crates/broker/broker/src/core/mod.rs`::`BrokerCore::register_session`, `BrokerCore::unregister_session` |
| PendingS2cReq | Pending server→client request | MUST be completed only by the elected responder | `crates/broker/broker/src/core/mod.rs`::`BrokerCore::register_client_request`, `BrokerCore::complete_client_request` |
| PendingC2sReq | Pending client→server request | MUST track origin session and original request id | `crates/broker/broker/src/core/mod.rs`::`BrokerCore::*` |
| LspProxyService | LSP stdio proxy and event forwarder | MUST register pending before forwarding request | `crates/broker/broker/src/lsp.rs`::`LspProxyService::call`, `LspProxyService::forward` |
| DocRegistry | URI → (DocId, version) tracking | MUST not report a doc that is not in by_uri | `crates/broker/broker/src/core/mod.rs`::`DocRegistry::update`, `BrokerCore::get_doc_by_uri` |
| DocOwnerRegistry | Single-writer ownership per URI | MUST transfer ownership on detach/unregister | `crates/broker/broker/src/core/mod.rs`::`BrokerCore::cleanup_session_docs_on_server` |
| Event | Broker → editor event stream | MUST include server_id for routing on client side | `crates/broker/broker/src/core/mod.rs`::`BrokerCore::broadcast_to_server`, `BrokerCore::send_to_leader` |

## Invariants (hard rules)
1. Project deduplication MUST use a stable ProjectKey; configs without cwd MUST not collapse unrelated projects.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`ProjectKey::from`
   - Tested by: `crates/broker/broker/src/core/tests.rs`::`project_dedup_*`
   - Failure symptom: unrelated projects share a server, causing incorrect diagnostics and cross-project symbol results.
2. Leader election MUST be deterministic and MUST be the minimum SessionId of the attached set.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`BrokerCore::attach_session`, `BrokerCore::detach_session`
   - Tested by: `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_leader_routing_and_reply`
   - Failure symptom: server-initiated requests route to different sessions across runs, breaking request handling and causing hangs.
3. Server→client requests MUST be registered as pending before being forwarded to the leader session.
   - Enforced in: `crates/broker/broker/src/lsp.rs`::`LspProxyService::call`
   - Tested by: `crates/broker/broker/src/core/tests.rs`::`reply_from_leader_completes_pending`
   - Failure symptom: leader reply arrives before pending registration and is rejected as "request not found".
4. Server→client requests MUST only be completed by the elected responder session.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`BrokerCore::complete_client_request`
   - Tested by: `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_leader_routing_and_reply`
   - Failure symptom: replies are accepted from non-leader sessions, resulting in nondeterministic behavior and incorrect responses.
5. Client→server request ids MUST be rewritten to broker-allocated wire ids to prevent cross-session collisions.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`BrokerCore::alloc_wire_request_id`
   - Tested by: `crates/editor/tests/broker_e2e.rs`::`test_broker_string_wire_ids`
   - Failure symptom: one session's response completes another session's request, causing incorrect editor UI and protocol errors.
6. Pending requests MUST be cancelled on leader change, session unregister, server exit, and per-request timeout.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`BrokerCore::cancel_pending_for_leader_change`, `BrokerCore::unregister_session`, `BrokerCore::check_lease_expiry`; `crates/broker/broker/src/lsp.rs`::`LspProxyService::call`
   - Tested by: `crates/broker/broker/src/core/tests.rs`::`disconnect_leader_cancels_pending_requests`
   - Failure symptom: pending maps leak, late replies are misdelivered, or server waits forever for a client reply.
7. IPC send failure to a session MUST trigger authoritative session cleanup.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`BrokerCore::broadcast_to_server`, `BrokerCore::send_to_leader`
   - Tested by: TODO (add regression: test_session_send_failure_unregisters_session)
   - Failure symptom: dead sessions remain registered; leader routing blackholes server-initiated requests.
8. Idle servers MUST be terminated after lease expiry only when no sessions are attached and no inflight requests exist.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`BrokerCore::check_lease_expiry`
   - Tested by: `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_persistence_lease_expiry`
   - Failure symptom: server processes leak indefinitely or are terminated while a request is still in flight.
9. On session unregister, broker MUST detach the session from all servers and MUST clean up per-session doc ownership state.
   - Enforced in: `crates/broker/broker/src/core/mod.rs`::`BrokerCore::unregister_session`, `BrokerCore::cleanup_session_docs_on_server`
   - Tested by: `crates/editor/tests/broker_e2e.rs`::`test_broker_owner_close_transfer`
   - Failure symptom: docs remain "owned" by a dead session, blocking updates from remaining sessions and causing stale diagnostics.
10. Diagnostics forwarding MUST prefer the authoritative version from the LSP payload when present, and MAY fall back to broker doc tracking otherwise.
    - Enforced in: `crates/broker/broker/src/lsp.rs`::`LspProxyService::forward`
    - Tested by: TODO (add regression: test_publish_diagnostics_version_fallback)
    - Failure symptom: diagnostics apply to the wrong document version, producing flicker or persistent stale errors.

## Data flow
1. Session connect: Editor connects to broker IPC socket and registers a SessionId and SessionSink.
2. Server start / attach: Editor requests LspStart for a project configuration. Broker deduplicates by ProjectKey; either starts a new server or attaches to an existing one.
3. Client→server messages: Editor sends notifications/requests for server_id. Broker rewrites request ids to wire ids and forwards to the LSP server process. Responses are mapped back to the origin session and request id via pending c2s map.
4. Server→client messages: LSP server sends: Notifications are broadcast to all attached sessions. Requests are registered as pending s2c and forwarded only to the leader session. Leader session replies; broker completes pending and returns the response to the LSP server.
5. Detach and lease: When the last session detaches, broker schedules lease expiry. If no new sessions attach and no inflight remains at expiry, broker terminates the server.

## Lifecycle
- Startup: Broker binary starts and initializes BrokerCore and IPC loop.
- Session registration: Each editor session registers with a SessionId and sink.
- Server registration: Broker starts or reuses an LSP server instance, assigns ServerId, attaches session, elects leader.
- Running: Broker proxies JSON-RPC in both directions and maintains pending request maps.
- Leader change: Detach of the leader triggers re-election to min(attached) and cancels pending s2c for the old leader.
- Idle lease: When attached is empty, broker schedules lease expiry; server remains warm until expiry conditions are met.
- Termination: On lease expiry with no inflight, or on explicit termination, broker stops the server and removes indices.
- Shutdown: Broker terminates all servers and clears state.

## Concurrency & ordering
- BrokerCore state access: BrokerCore serializes state mutation behind its state lock. All state-dependent routing decisions (leader selection, attachment membership, pending maps) MUST be made under that lock.
- Pending request ordering: Server→client requests are routed to leader and completed by matching request id. Client implementations MUST preserve FIFO request/reply pairing if they use a queue-based strategy.
- Background tasks: Lease expiry runs in a spawned task and MUST re-check generation tokens to avoid stale termination. Server monitor tasks MUST report exits and trigger cleanup.

## Failure modes & recovery
- Session IPC disconnect: Broker detects send failure and unregisters session; pending requests for that session are cancelled.
- Leader disconnect: Broker cancels pending s2c requests for the old leader and elects a new leader if possible.
- Server crash: Broker marks server stopped, cancels inflight, and removes server indices; subsequent start attaches to a fresh server.
- Request timeout (server→client): Broker cancels pending and replies with REQUEST_CANCELLED error to the server.
- Dedup mismatch: If ProjectKey construction is wrong, broker shares servers incorrectly; fix ProjectKey normalization and add regression tests.

## Recipes
### Add a new broker IPC event
- Extend xeno_broker_proto::types::Event.
- Update broker broadcast/send sites to emit the new event.
- Update the editor transport event mapping to surface it as a TransportEvent or UI event.

### Debug broker routing issues with file logs
- Set XENO_LOG_DIR and RUST_LOG in the editor environment.
- Ensure editor spawns broker with env propagation.
- Inspect xeno-broker.<pid>.log for: attach/detach and leader re-election logs, pending map registration/completion/cancellation, lease scheduling and termination decisions.

### Verify multi-process dedup
- Run two editors against the same workspace concurrently.
- Confirm broker spawns one server process and attaches both sessions to the same ServerId.

## Tests
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_persistence_lease_expiry`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_persistence_warm_reattach`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_leader_routing_and_reply`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_e2e_dedup_and_fanout`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_reconnect_wedge`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_owner_close_transfer`
- `crates/editor/tests/broker_e2e.rs`::`test_broker_string_wire_ids`
- `crates/broker/broker/src/core/tests.rs`::`test_lease_expiry_terminates_server`
- `crates/broker/broker/src/core/tests.rs`::`test_warm_reattach_reuses_server`
- `crates/broker/broker/src/core/tests.rs`::`reply_from_leader_completes_pending`
- `crates/broker/broker/src/core/tests.rs`::`disconnect_leader_cancels_pending_requests`
- TODO (add regression: test_session_send_failure_unregisters_session)
- TODO (add regression: test_publish_diagnostics_version_fallback)

## Glossary
- attach: The act of associating a SessionId with a ServerId so it receives broadcast events and can send requests.
- broker: The daemon that owns LSP server processes and routes JSON-RPC between servers and editor sessions.
- leader: The minimum SessionId among attached sessions for a server; the only session that receives server-initiated requests.
- lease: A duration during which an idle server is kept alive after the last detach, before termination.
- pending c2s: Broker map of client→server requests awaiting server response, used to map wire ids back to origin session and id.
- pending s2c: Broker map of server→client requests awaiting editor reply, routed only to the leader session.
- project key: Deduplication identity derived from server command, args, and cwd (with a no-cwd sentinel).
- session: One connected editor instance, identified by SessionId, with an IPC sink for outbound events.
- server id: Broker-assigned identifier for a managed language server instance.
- wire request id: Broker-allocated request id used on the broker↔server connection to avoid collisions between sessions.
