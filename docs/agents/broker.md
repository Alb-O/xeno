# Broker

## Purpose
- Define the broker daemon that deduplicates, shares, and supervises language server processes across editor sessions.
- Describe broker-side routing rules for server↔client JSON-RPC, including leader election, pending request tracking, and lease-based persistence.
- Define cross-process buffer synchronization: single-writer model where the broker maintains an authoritative rope, the owner session publishes deltas, and the broker validates, applies, and broadcasts to follower sessions.
- Exclude editor-side document sync and UI integration; see docs/agents/lsp.md.

## Mental model
- The broker is an out-of-process daemon that owns the actual LSP server processes.
- Editor sessions connect to the broker via IPC and register a SessionId.
- Each LSP server instance is keyed by ProjectKey and shared across sessions that attach to that server.
- Server→client requests are routed only to the leader session (deterministic: minimum SessionId).
- Client→server requests are rewritten to broker-allocated wire request ids to avoid collisions between sessions.
- The broker keeps idle servers alive for an idle lease duration; after lease expiry and with no inflight requests, the server is terminated.
- For buffer sync: each document URI has exactly one owner session. The owner sends deltas; the broker validates epoch/sequence, applies to its authoritative rope, and broadcasts to all other sessions (followers). Ownership transfers on disconnect or explicit request, bumping the epoch and resetting the sequence.

## Module map
- `broker` — Public crate exports and module declarations.
- `broker::core` — BrokerCore state machine: sessions, servers, project deduplication, leader election, pending request maps, leases, and buffer sync document state (`sync_docs`).
- `broker::service` — BrokerService: per-connection IPC request handler that routes editor requests to BrokerCore or LSP servers.
- `broker::ipc` — IPC server (Unix socket listener) and per-connection dispatch to BrokerService.
- `broker::lsp` — LspProxyService: proxy between an LSP server stdio connection and BrokerCore event routing.
- `broker::launcher` — Spawns and monitors LSP server child processes and reports exit to core.
- `broker::wire_convert` — Conversion between `Transaction`/`ChangeSet`/`Operation` and `WireTx`/`WireOp` wire formats.
- `broker_bin::main` — Broker binary entrypoint and file-based tracing setup (xeno-broker.<pid>.log).
- `editor::lsp::broker_transport` — Broker daemon spawn logic, environment propagation, and buffer sync event channel wiring.
- `editor::buffer_sync` — Editor-side `BufferSyncManager`: tracks per-document sync state, converts transactions to/from wire format, and provides helpers for the editor lifecycle.
- `broker_proto` — IPC frame and event types: IpcFrame, Event, Request, Response, LspServerConfig, ServerId, SessionId, SyncEpoch, SyncSeq, WireOp, WireTx, BufferSyncRole.

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| BrokerCore | Authoritative broker state machine | MUST be the only owner of session/server maps | `BrokerCore::*` |
| ProjectKey | Dedup key for LSP servers | MUST uniquely represent command/args/cwd (with no-cwd sentinel) | `ProjectKey::from` |
| ServerEntry | One managed LSP server instance | MUST maintain leader = min(attached) | `BrokerCore::attach_session`, `BrokerCore::detach_session` |
| SessionEntry | One connected editor session | MUST track attachment set for cleanup | `BrokerCore::register_session`, `BrokerCore::unregister_session` |
| PendingS2cReq | Pending server→client request | MUST be completed only by the elected responder | `BrokerCore::register_client_request`, `BrokerCore::complete_client_request` |
| PendingC2sReq | Pending client→server request | MUST track origin session and original request id | `BrokerCore::*` |
| LspProxyService | LSP stdio proxy and event forwarder | MUST register pending before forwarding request | `LspProxyService::call`, `LspProxyService::forward` |
| DocRegistry | URI → (DocId, version) tracking | MUST not report a doc that is not in by_uri | `DocRegistry::update`, `BrokerCore::get_doc_by_uri` |
| DocOwnerRegistry | Single-writer ownership per URI | MUST transfer ownership on detach/unregister | `BrokerCore::cleanup_session_docs_on_server` |
| SyncDocState | Per-URI broker-authoritative sync state | MUST have exactly one owner; epoch increments on ownership change; seq increments on delta | `BrokerCore::on_buffer_sync_open`, `BrokerCore::on_buffer_sync_delta`, `BrokerCore::on_buffer_sync_close` |
| SyncEpoch | Monotonic ownership generation | MUST increment on every ownership transfer | `BrokerCore::on_buffer_sync_take_ownership`, `BrokerCore::on_buffer_sync_close` |
| SyncSeq | Monotonic edit sequence within an epoch | MUST increment on every applied delta; resets to 0 on epoch change | `BrokerCore::on_buffer_sync_delta` |
| BufferSyncManager | Editor-side per-document sync tracker | MUST clear all state on broker disconnect | `BufferSyncManager::disable_all` |
| Event | Broker → editor event stream | MUST include server_id for routing on client side; buffer sync events MUST include URI | `BrokerCore::broadcast_to_server`, `BrokerCore::send_to_leader`, `BrokerCore::broadcast_to_sync_doc_sessions` |

## Invariants (hard rules)
1. Project deduplication MUST use a stable ProjectKey; configs without cwd MUST not collapse unrelated projects.
   - Enforced in: `ProjectKey::from`
   - Tested by: `core::tests::project_dedup_*`
   - Failure symptom: unrelated projects share a server, causing incorrect diagnostics and cross-project symbol results.
2. Leader election MUST be deterministic and MUST be the minimum SessionId of the attached set.
   - Enforced in: `BrokerCore::attach_session`, `BrokerCore::detach_session`
   - Tested by: `test_broker_e2e_leader_routing_and_reply`
   - Failure symptom: server-initiated requests route to different sessions across runs, breaking request handling and causing hangs.
3. Server→client requests MUST be registered as pending before being forwarded to the leader session.
   - Enforced in: `LspProxyService::call`
   - Tested by: `core::tests::request_routing::reply_from_leader_completes_pending`
   - Failure symptom: leader reply arrives before pending registration and is rejected as "request not found".
4. Server→client requests MUST only be completed by the elected responder session.
   - Enforced in: `BrokerCore::complete_client_request`
   - Tested by: `test_broker_e2e_leader_routing_and_reply`
   - Failure symptom: replies are accepted from non-leader sessions, resulting in nondeterministic behavior and incorrect responses.
5. Client→server request ids MUST be rewritten to broker-allocated wire ids to prevent cross-session collisions.
   - Enforced in: `BrokerCore::alloc_wire_request_id`
   - Tested by: `test_broker_string_wire_ids`
   - Failure symptom: one session's response completes another session's request, causing incorrect editor UI and protocol errors.
6. Pending requests MUST be cancelled on leader change, session unregister, server exit, and per-request timeout.
   - Enforced in: `BrokerCore::cancel_pending_for_leader_change`, `BrokerCore::unregister_session`, `BrokerCore::check_lease_expiry`, `LspProxyService::call`
   - Tested by: `core::tests::request_routing::disconnect_leader_cancels_pending_requests`
   - Failure symptom: pending maps leak, late replies are misdelivered, or server waits forever for a client reply.
7. IPC send failure to a session MUST trigger authoritative session cleanup.
   - Enforced in: `BrokerCore::broadcast_to_server`, `BrokerCore::send_to_leader`
   - Tested by: `core::tests::error_handling::session_send_failure_unregisters_session`
   - Failure symptom: dead sessions remain registered; leader routing blackholes server-initiated requests.
8. Idle servers MUST be terminated after lease expiry only when no sessions are attached and no inflight requests exist.
   - Enforced in: `BrokerCore::check_lease_expiry`
   - Tested by: `test_broker_e2e_persistence_lease_expiry`
   - Failure symptom: server processes leak indefinitely or are terminated while a request is still in flight.
9. On session unregister, broker MUST detach the session from all servers and MUST clean up per-session doc ownership state.
   - Enforced in: `BrokerCore::unregister_session`, `BrokerCore::cleanup_session_docs_on_server`
   - Tested by: `test_broker_owner_close_transfer`
   - Failure symptom: docs remain "owned" by a dead session, blocking updates from remaining sessions and causing stale diagnostics.
10. Diagnostics forwarding MUST prefer the authoritative version from the LSP payload when present, and MAY fall back to broker doc tracking otherwise.
    - Enforced in: `LspProxyService::forward`
    - Tested by: `core::tests::diagnostics_regression::diagnostics_use_lsp_payload_version_not_broker_version`
    - Failure symptom: diagnostics apply to the wrong document version, producing flicker or persistent stale errors.
11. Buffer sync deltas MUST be rejected if the sender is not the owner or epoch/seq do not match.
    - Enforced in: `BrokerCore::on_buffer_sync_delta`
    - Tested by: `core::tests::buffer_sync::test_buffer_sync_rejects_non_owner`, `core::tests::buffer_sync::test_buffer_sync_seq_mismatch_triggers_resync`
    - Failure symptom: follower sessions overwrite the authoritative rope, causing document divergence.
12. Buffer sync ownership MUST transfer to the minimum remaining SessionId when the owner disconnects or closes the document.
    - Enforced in: `BrokerCore::on_buffer_sync_close`, `BrokerCore::cleanup_session_sync_docs`
    - Tested by: `core::tests::buffer_sync::test_buffer_sync_owner_disconnect_elects_successor_epoch_bumps`
    - Failure symptom: no session holds ownership after disconnect, blocking all edits until manual resync.
13. Buffer sync epoch MUST increment on every ownership transfer; sequence MUST reset to 0.
    - Enforced in: `BrokerCore::on_buffer_sync_take_ownership`, `BrokerCore::on_buffer_sync_close`
    - Tested by: `core::tests::buffer_sync::test_buffer_sync_take_ownership`
    - Failure symptom: stale-epoch deltas are accepted, applying edits from a previous ownership era.
14. Buffer sync broadcast MUST exclude the sender session and MUST include all other sessions with open refcounts for the URI.
    - Enforced in: `BrokerCore::broadcast_to_sync_doc_sessions`
    - Tested by: `core::tests::buffer_sync::test_buffer_sync_delta_ack_and_broadcast`
    - Failure symptom: sender receives its own delta as a remote edit (infinite loop), or some followers miss deltas.
15. On broker disconnect, the editor MUST clear all buffer sync state and remove all follower readonly overrides.
    - Enforced in: `Editor::handle_buffer_sync_disconnect`
    - Tested by: TODO (add regression: test_buffer_sync_disconnect_clears_readonly)
    - Failure symptom: buffers remain stuck in readonly mode after broker disconnect, blocking local editing.

## Data flow

### LSP routing
1. Session connect: Editor connects to broker IPC socket and registers a SessionId and SessionSink.
2. Server start / attach: Editor requests LspStart for a project configuration. Broker deduplicates by ProjectKey; either starts a new server or attaches to an existing one.
3. Client→server messages: Editor sends notifications/requests for server_id. Broker rewrites request ids to wire ids and forwards to the LSP server process. Responses are mapped back to the origin session and request id via pending c2s map.
4. Server→client messages: LSP server sends: Notifications are broadcast to all attached sessions. Requests are registered as pending s2c and forwarded only to the leader session. Leader session replies; broker completes pending and returns the response to the LSP server.
5. Detach and lease: When the last session detaches, broker schedules lease expiry. If no new sessions attach and no inflight remains at expiry, broker terminates the server.

### Buffer sync
1. Document open: Editor sends `BufferSyncOpen { uri, text }`. First opener becomes Owner with epoch=1, seq=0. Subsequent openers become Followers and receive a snapshot of the current content.
2. Local edit (owner path): Editor applies transaction locally, then calls `BufferSyncManager::prepare_delta` which serializes to `WireTx` and sends `BufferSyncDelta { uri, epoch, base_seq, tx }` to the broker via fire-and-forget channel.
3. Broker delta processing: Broker validates owner/epoch/seq, converts `WireTx` to `Transaction`, applies to authoritative rope, increments seq, broadcasts `Event::BufferSyncDelta` to all followers, and replies with `DeltaAck { seq }`.
4. Remote delta (follower path): Editor receives `BufferSyncEvent::RemoteDelta`, converts wire tx back to `Transaction`, applies with `UndoPolicy::NoUndo`, and maps selections for all views of the document.
5. Ownership change: On owner disconnect or explicit `TakeOwnership`, broker bumps epoch, resets seq, broadcasts `Event::BufferSyncOwnerChanged`. New owner becomes writable; old owner (if still connected) becomes follower (readonly).
6. Document close: Editor sends `BufferSyncClose`. Broker decrements refcount; if owner closed, elects successor (min session ID). Last close removes the entry.
7. Disconnect recovery: On broker transport disconnect, editor calls `BufferSyncManager::disable_all()` and clears all follower readonly overrides.

## Lifecycle
- Startup: Broker binary starts and initializes BrokerCore and IPC loop.
- Session registration: Each editor session registers with a SessionId and sink.
- Server registration: Broker starts or reuses an LSP server instance, assigns ServerId, attaches session, elects leader.
- Running: Broker proxies JSON-RPC in both directions and maintains pending request maps. Buffer sync deltas are validated and applied to the authoritative rope.
- Leader change: Detach of the leader triggers re-election to min(attached) and cancels pending s2c for the old leader.
- Buffer sync open: Editor calls `BufferSyncOpen` during buffer lifecycle; broker creates or joins a `SyncDocState`.
- Buffer sync ownership transfer: On owner disconnect or explicit request, broker bumps epoch, broadcasts `OwnerChanged`, new owner starts publishing deltas.
- Buffer sync close: Editor calls `BufferSyncClose` during buffer removal; broker decrements refcount and elects successor if needed.
- Idle lease: When attached is empty, broker schedules lease expiry; server remains warm until expiry conditions are met.
- Session cleanup: `cleanup_session_sync_docs` removes the disconnected session from all sync docs and transfers ownership as needed.
- Termination: On lease expiry with no inflight, or on explicit termination, broker stops the server and removes indices.
- Shutdown: Broker terminates all servers and clears state.

## Concurrency & ordering
- BrokerCore state access: BrokerCore serializes state mutation behind its state lock. All state-dependent routing decisions (leader selection, attachment membership, pending maps) MUST be made under that lock.
- Pending request ordering: Server→client requests are routed to leader and completed by matching request id. Client implementations MUST preserve FIFO request/reply pairing if they use a queue-based strategy.
- Background tasks: Lease expiry runs in a spawned task and MUST re-check generation tokens to avoid stale termination. Server monitor tasks MUST report exits and trigger cleanup.

## Failure modes & recovery
- Session IPC disconnect: Broker detects send failure and unregisters session; pending requests for that session are cancelled; buffer sync docs owned by this session transfer ownership.
- Leader disconnect: Broker cancels pending s2c requests for the old leader and elects a new leader if possible.
- Server crash: Broker marks server stopped, cancels inflight, and removes server indices; subsequent start attaches to a fresh server.
- Request timeout (server→client): Broker cancels pending and replies with REQUEST_CANCELLED error to the server.
- Dedup mismatch: If ProjectKey construction is wrong, broker shares servers incorrectly; fix ProjectKey normalization and add regression tests.
- Buffer sync epoch mismatch: Follower delta rejected with `SyncEpochMismatch`; editor should request resync.
- Buffer sync seq mismatch: Delta rejected with `SyncSeqMismatch`; editor should request resync to recover.
- Broker disconnect (editor side): Editor clears all sync state via `disable_all()` and removes readonly overrides so local editing resumes.

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

### Verify buffer sync two-terminal
- Open the same file in two terminal windows.
- Type in one window; confirm the other receives the edit in real-time.
- Close the owner terminal; confirm the follower terminal becomes the new owner and can edit.

### Add a new buffer sync event
- Add the variant to `BufferSyncEvent` in `editor::buffer_sync::mod`.
- Handle it in `BrokerClientService::notify()` in `editor::lsp::broker_transport`.
- Dispatch it in `Editor::handle_buffer_sync_event()` in `editor::impls::buffer_sync_events`.

## Tests

### LSP routing
- `test_broker_e2e_persistence_lease_expiry`
- `test_broker_e2e_persistence_warm_reattach`
- `test_broker_e2e_leader_routing_and_reply`
- `test_broker_e2e_dedup_and_fanout`
- `test_broker_reconnect_wedge`
- `test_broker_owner_close_transfer`
- `test_broker_string_wire_ids`
- `core::tests::lease_management::lease_expiry_terminates_server`
- `core::tests::lease_management::warm_reattach_reuses_server`
- `core::tests::request_routing::reply_from_leader_completes_pending`
- `core::tests::request_routing::disconnect_leader_cancels_pending_requests`
- `core::tests::error_handling::session_send_failure_unregisters_session`
- `core::tests::diagnostics_regression::diagnostics_use_lsp_payload_version_not_broker_version`

### Wire conversion
- `wire_convert::tests::round_trip_retain_only`
- `wire_convert::tests::round_trip_insert`
- `wire_convert::tests::round_trip_delete`
- `wire_convert::tests::round_trip_mixed_ops`
- `wire_convert::tests::round_trip_unicode`
- `wire_convert::tests::wire_ops_are_correct`

### Buffer sync
- `core::tests::buffer_sync::test_buffer_sync_open_owner_then_follower_gets_snapshot`
- `core::tests::buffer_sync::test_buffer_sync_rejects_non_owner`
- `core::tests::buffer_sync::test_buffer_sync_seq_mismatch_triggers_resync`
- `core::tests::buffer_sync::test_buffer_sync_owner_disconnect_elects_successor_epoch_bumps`
- `core::tests::buffer_sync::test_buffer_sync_delta_ack_and_broadcast`
- `core::tests::buffer_sync::test_buffer_sync_broadcast_matches_broker_rope`
- `core::tests::buffer_sync::test_buffer_sync_take_ownership`
- `core::tests::buffer_sync::test_buffer_sync_close_last_session_removes_doc`
- `core::tests::buffer_sync::test_buffer_sync_resync_returns_snapshot`

## Glossary
- attach: The act of associating a SessionId with a ServerId so it receives broadcast events and can send requests.
- authoritative rope: The broker-side ropey::Rope that represents the ground truth for a synced document; all deltas are validated against it.
- broker: The daemon that owns LSP server processes and routes JSON-RPC between servers and editor sessions.
- buffer sync: Cross-process document synchronization subsystem where one owner publishes deltas and multiple followers receive them via the broker.
- delta: A serialized edit transaction (`WireTx`) sent from the owner to the broker for validation and broadcast.
- epoch: Monotonic ownership generation (`SyncEpoch`); increments on every ownership transfer, resets sequence to 0.
- follower: A session that has a synced document open but does not own it; receives remote deltas and has readonly override set.
- leader: The minimum SessionId among attached sessions for a server; the only session that receives server-initiated requests.
- lease: A duration during which an idle server is kept alive after the last detach, before termination.
- owner: The session that holds write authority for a synced document; publishes deltas to the broker.
- pending c2s: Broker map of client→server requests awaiting server response, used to map wire ids back to origin session and id.
- pending s2c: Broker map of server→client requests awaiting editor reply, routed only to the leader session.
- project key: Deduplication identity derived from server command, args, and cwd (with a no-cwd sentinel).
- sequence: Monotonic edit counter (`SyncSeq`) within an epoch; increments on each applied delta.
- session: One connected editor instance, identified by SessionId, with an IPC sink for outbound events.
- server id: Broker-assigned identifier for a managed language server instance.
- wire request id: Broker-allocated request id used on the broker↔server connection to avoid collisions between sessions.
- wire tx: Serialized transaction format (`WireTx = Vec<WireOp>`) used for IPC; each op is Retain/Delete/Insert.
