# LSP System Architecture

## Purpose
- Owns: LSP client stack, JSON-RPC transport, document sync policy, and feature-specific controllers.
- Does not own: Document content (owned by `Document`), UI presentation (delegated via events).
- Source of truth: `LspSystem` in `crates/editor/src/lsp/system.rs` (wraps `xeno_lsp::LspManager`).

## Mental model
- Terms: Client (server instance), Sync (didOpen/didChange), Generation (staleness token), Outbox (single writer).
- Lifecycle in one sentence: Buffers open a client, edits queue in a sync manager, and async results are validated by generation before applying.

## Module map
- `crates/lsp/` — Core framework, transport, and protocol implementation.
- `crates/editor/src/lsp/system.rs` — Integration root and public API.
- `crates/editor/src/lsp/sync_manager/` — Coalescing and sync logic.
- `crates/editor/src/lsp/events.rs` — UI event fanout and state application.

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| `ClientHandle` | Communication channel | MUST check `Ready` state | `crates/editor/src/lsp/system.rs`::`LspSystem::on_buffer_open` |
| `LspUiEvent` | Async UI result | Validated by generation | `crates/editor/src/lsp/events.rs`::`handle_lsp_ui_event` |
| `DocSyncState` | Server's view of doc | MUST only apply changes in-order | `crates/editor/src/lsp/sync_manager/mod.rs`::`LspSyncManager` |

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

## Data flow
1. Trigger: User types or triggers manual completion.
2. Request: `LspSystem` builds request with current generation/version.
3. Async boundary: `xeno_lsp` sends JSON-RPC; awaits response.
4. Validation: Result matches current generation and buffer version?
5. Effect: Apply edits or open menu via `LspUiEvent`.

## Lifecycle
- Starting: Server process spawning and initializing.
- Ready: Handshake complete; accepting edits/requests.
- Dead: Process crashed or shutdown; registry will clean up.
- Note: A `Dead` client may be restarted by the registry on the next `get_or_start` call.

## Concurrency & ordering
- Single-writer Outbox: Only one task writes to the socket.
- Monotonic Generations: Every completion/signature request has a unique incrementing ID.

## Failure modes & recovery
- Server Crash: Mark client `Dead` and suppress features; restart on next demand.
- Unsupported Capability: `prepare_position_request` returns `None`; feature is suppressed.
- Canonicalization Failure: Skip LSP operations for that buffer to avoid identity aliasing.

## Recipes
### Add a new LSP feature
Steps:
- Add typed method to `xeno_lsp::client::api`.
- Implement prepare/send logic in `LspSystem`.
- Handle result/notification in `LspEventHandler` or `drain_lsp_ui_events`.

## Tests
- `crates/editor/src/lsp/sync_manager/tests.rs`::`test_doc_open_close`
- `crates/editor/src/lsp/sync_manager/tests.rs`::`test_contiguity_check_success`
- `crates/editor/src/lsp/sync_manager/tests.rs`::`test_write_timeout_escalates_to_full`

## Glossary
- Client: A running instance of a Language Server.
- Sync: The protocol for keeping the server's view of a document in sync with the editor.
- Generation: A unique identifier for a specific UI request to prevent applying stale results.
- Outbox: The bounded queue of outgoing LSP messages.
