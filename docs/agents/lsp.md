# Xeno LSP System Architecture

## Scope / crate split

Two-layer LSP stack; transport/client core is editor-agnostic, editor owns doc semantics.

| Layer                  | Location                 | Owns                                                                        | Must NOT own                                                                       |
| ---------------------- | ------------------------ | --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| LSP framework + client | `crates/lsp/`            | JSON-RPC IO, request correlation, lifecycle state machine, caps, typed API  | document versioning, debounce, incremental-vs-full policy, pre-init edit buffering |
| Editor integration     | `crates/editor/src/lsp/` | doc sync policy, batching, retry, UI/event fanout, buffer/document bridging | process spawn plumbing, RPC multiplexing internals                                 |

---

## Hard invariants (what correctness is built on)

### Client-side (protocol/lifecycle)

* **No implicit pre-init buffering**: if `ServerState != Ready`, API returns `Error::NotReady` (or caller must `wait_ready()`).
* **Single-writer outbox**: exactly one task writes bytes to socket; all outbound traffic goes through `Outbox`.
* **Total request correlation**: every sent request either completes a pending entry or triggers `ProtocolError` (unknown/duplicate IDs).
* **Shutdown drains pending**: state→Dead forces all pending requests to resolve `ServiceStopped` (no hangs).
* **Ready transition is explicit**: `initialize` request → set caps → `initialized` notification → *write barrier completes* → state = Ready.

### Editor-side (document correctness)

* **SyncManager is the only edit buffer**: edits accumulate locally while server Starting/Dead/backpressured.
* **Contiguity is explicit**: each commit carries `(prev_version, new_version)`; mismatch => force full.
* **Incremental chain is valid or discarded**: if you can’t prove “server baseline version + contiguous commits”, you full-sync.
* **“Clean” means “no unsent local deltas”** (transmitted), not “server processed notification” (not observable for notifications).

---

## Module map (post-refactor target)

### `crates/lsp/` (xeno-lsp)

Top-level core remains Tower-oriented (router/mainloop/middleware), but client internals become explicitly partitioned:

```
src/
  lib.rs               // public API + crate Error enum
  types.rs             // AnyRequest/AnyNotification/AnyResponse (internal)
  message.rs           // JSON-RPC framing
  socket.rs            // ClientSocket/ServerSocket
  router.rs            // method dispatch (req/notification/events)
  mainloop.rs          // driver: read loop + dispatch + write hooks
  registry.rs          // server dedupe + config -> spawn
  sync.rs              // DocumentSync coordinator
  position.rs, changes.rs // offset encoding + change computation (pure)

  client/
    mod.rs             // exports + re-exports
    handle.rs          // ClientHandle + wait_ready()
    state.rs           // ServerState enum
    lifecycle.rs       // spawn + init handshake + stdio process management
    outbox.rs          // OutboundMsg + WriteBarrier + outbound_dispatcher
    router_setup.rs    // install handlers (diagnostics/progress/log)
    api.rs             // typed request/notification façade
    capabilities.rs    // client_capabilities() builder
    config.rs          // LanguageServerId, ServerConfig, OffsetEncoding
    event_handler.rs   // LspEventHandler trait + NoOpEventHandler
```

### `crates/editor/src/lsp/`

```
src/lsp/
  mod.rs               // LspManager: integration root
  system.rs            // LspSystem: top-level coordinator (owns managers + controllers)
  sync_manager.rs      // doc sync policy + state machine (thresholds inline)
  events.rs            // LspUiEvent fanout
  diagnostics.rs       // diagnostic navigation
  completion.rs        // completion lifecycle + application
  completion_controller.rs // debounce + cancellation
  completion_filter.rs // fuzzy matching
  code_action.rs       // code action menu
  signature_help.rs    // function signature display
  menu.rs              // shared navigation logic
  snippet.rs           // LSP snippet parsing
  workspace_edit.rs    // multi-file edit application
  coalesce/            // optional delta coalescing
```

---

## Key types (only the ones that matter for invariants)

### Client lifecycle / handle

```rust
pub enum ServerState { Starting, Ready, Dead }

#[derive(Clone)]
pub struct ClientHandle {
  id: LanguageServerId,
  name: String,
  capabilities: Arc<OnceCell<ServerCapabilities>>,
  root_path: PathBuf,
  root_uri: Option<Uri>,
  initialize_notify: Arc<Notify>,
  outbound_tx: mpsc::Sender<OutboundMsg>,
  timeout: Duration,
  state_tx: watch::Sender<ServerState>,
}

impl ClientHandle {
  pub async fn wait_ready(&self) -> Result<()>;  // blocks until Ready or Dead
  pub fn state(&self) -> ServerState;
  pub fn try_capabilities(&self) -> Option<&ServerCapabilities>;
  pub fn is_initialized(&self) -> bool;
  pub fn is_ready(&self) -> bool;
}
```

### Outbox + barriers (write fence only)

```rust
pub(super) type WriteBarrier = oneshot::Sender<()>;

pub(super) enum OutboundMsg {
  Notification { notification: AnyNotification, barrier: Option<WriteBarrier> },
  Request { request: AnyRequest, response_tx: oneshot::Sender<AnyResponse> },
}
```

Barrier semantics: fires after successful serialization + socket write returns. Nothing stronger. Only notifications support barriers; requests get their fence via the response round-trip.

### Pending request map

Request correlation is handled by the mainloop via outbound channel. Semantics:
* ID allocation is monotonic per request.
* Router completes via response matching.
* Dead state drains pending with `ServiceStopped`.

### Editor edit payload (explicit contiguity)

Edits flow to `LspSyncManager.on_doc_edit()` with explicit versioning:

```rust
fn on_doc_edit(&mut self, doc_id, prev_version: u64, new_version: u64, changes: Vec<LspDocumentChange>, bytes: usize)
```

### SyncManager doc state (contiguous chain or full)

```rust
pub struct DocSyncState {
  config: LspDocumentConfig,    // path, language, supports_incremental
  open_sent: bool,
  needs_full: bool,
  pending_changes: Vec<LspDocumentChange>,
  pending_bytes: usize,

  phase: SyncPhase,
  last_edit_at: Instant,
  retry_after: Option<Instant>,

  editor_version: u64,          // latest seen new_version
  expected_prev: Option<u64>,   // contiguity guard (prev_version)
  inflight: Option<InFlightInfo>,
}

pub struct InFlightInfo {
  is_full: bool,
  version: u64,        // version being sent (new_version)
  started_at: Instant, // for write-timeout only
}

pub enum SyncPhase { Idle, Debouncing, InFlight }
```

---

## Fence model (don’t invent server acks)

Two fence strengths, only one is explicit:

* **WriteFence**: `WriteBarrier` completion; proves bytes written to transport.
* **ProcessFence**: *implicit* via any request/response round-trip after flushing. Use this when you need “server has observed updates enough to answer.”

Rule: didChange cleanliness uses WriteFence; feature correctness uses ProcessFence (request response).

---

## Data flow (Editor → Server)

Edits are never dropped; they queue in SyncManager until ready.

```
Document tx commit → (prev_version, new_version, changes, bytes)
  ↓
LspSyncManager.on_doc_edit(doc_id, prev, new, changes, bytes)
  - contiguity: expected_prev == prev_version ? else needs_full=true; pending_changes=∅
  - threshold: bytes/ops -> needs_full=true (optionally discard incremental list)
  - append incremental changes if allowed
  - phase = Debouncing, last_edit_at=now
  ↓
tick(now, client_ready, sync, metrics, snapshot_provider)
  - poll completions (FlushComplete messages)
  - check write timeouts (inflight.started_at)
  - if doc due and not InFlight:
      take_for_send() => payload(full|incremental), phase=InFlight
      spawn async send task:
         1) full: get snapshot via provider, send didOpen/didChange via DocumentSync
         2) incremental: coalesce changes, send didChange via DocumentSync
         3) on barrier/completion => FlushComplete(doc_id, result, was_full)
  ↓
mark_complete(result, was_full)
  - Success: clear pending; phase=Idle; inflight=None; expected_prev=Some(version)
  - Retryable: preserve pending; retry_after=now+delay; phase=Debouncing
  - Failed: pending cleared; needs_full=true; phase=Debouncing
```

---

## Data flow (Server → Editor)

Same as before: socket read → router → event handler → editor state.

```
server notification
  ↓
mainloop read + parse
  ↓
router dispatch by method
  - publishDiagnostics -> DocumentStateManager.update_diagnostics -> UI
  - $/progress -> progress manager -> UI
  - window/logMessage/showMessage -> event handler -> UI/log
```

Note: diagnostics are *not* a reliable “applied version ack”; treat as UI input only.

---

## Server lifecycle (Registry + client lifecycle)

### Startup (Registry-owned)

* `Registry::get_or_start(language, file_path)`:

  * resolve root via markers
  * dedupe by `(language, root_path)`
  * spawn server process
  * create `ClientHandle` with `ServerState::Starting`
  * lifecycle task runs init handshake

### Init handshake (Client lifecycle-owned)

1. spawn stdio transport + socket tasks
2. send `initialize` request; await response; store `ServerCapabilities`
3. send `initialized` notification with barrier
4. barrier completes → `ServerState::Ready`
5. Ready waiters released

### Crash/shutdown

* transport monitor detects exit/EOF → state=Dead
* pending drained -> `ServiceStopped`
* registry discards dead instance; next `get_or_start()` respawns
* SyncManager sees send failures and forces full on next attempt

---

## Error taxonomy (flush policy mapping)

Client errors map to flush outcomes:

| Error variant         | FlushResult | Sync action                  |
| --------------------- | ----------- | ---------------------------- |
| `NotReady`            | Retryable   | keep pending; retry_after    |
| `Backpressure`        | Retryable   | keep pending; retry_after    |
| `ServiceStopped`      | Failed      | force full; retry_after      |
| `Protocol(_)`         | Failed      | force full; log loud         |
| `ServerSpawn{..}`     | Failed      | force full; wait for respawn |
| `Io(_)` / `Eof`       | Failed      | force full; wait for respawn |

Write timeout (inflight too old): treat as Failed → full.

---

## Constants (policy knobs)

Editor sync (`crates/editor/src/lsp/sync_manager.rs`):

* `LSP_DEBOUNCE` (30ms) - debounce window
* `LSP_MAX_INCREMENTAL_CHANGES` (100) - max ops before full
* `LSP_MAX_INCREMENTAL_BYTES` (100KB) - max bytes before full
* `LSP_ERROR_RETRY_DELAY` (250ms) - retry delay
* `LSP_WRITE_TIMEOUT` (10s) - write timeout
* `LSP_MAX_DOCS_PER_TICK` (8) - docs-per-tick budget

Client outbox (`crates/lsp/src/client/outbox.rs`):

* `OUTBOUND_QUEUE_LEN` (256) - bounded queue (backpressure is explicit)

Registry:

* request timeout defaults (per ServerConfig)
* root marker list per server config

---

## Cargo feature surface (xeno-lsp)

* `client` - spawning/registry/handle (enables tokio, sync, process)
* `position` - offset conversion + DocumentSync (implies `client`, adds ropey)
* `client-monitor` - client process monitor middleware (default)
* `omni-trait` - LanguageServer/LanguageClient mega-traits (default)
* `stdio` - stdin/stdout utilities for servers (default)
* `forward` - LspService impl for sockets

---

## Updated invariants list

1. **No pre-init send**: client refuses unless Ready.
2. **Edits never lost**: SyncManager accumulates until Ready.
3. **Contiguity is validated**: commit carries `(prev,new)`; mismatch => full.
4. **Write ordering**: single-writer + barrier ensures FIFO at transport.
5. **No phantom “server ack”**: notifications don’t ack; use request response as process fence when required.
6. **Dedupe**: one server per `(language, root_path)` (registry).
7. **Timeout safety**: inflight write timeout breaks deadlocks.
8. **Backpressure resilience**: bounded outbox + retry scheduling.

---

## Component graph (ownership edges)

```
Editor
  Document.apply_tx -> (prev, new, changes, bytes)
  LspSyncManager (buffers + policy)
    -> tick() with client_ready check
    -> DocumentSync (open/change/close coordination)
LSP crate
  ClientHandle
    -> Outbox (single writer) -> ServerSocket -> process stdio
  Router + MainLoop
    -> response matching
    -> LspEventHandler -> editor events
Registry
  -> lifecycle spawn/init via start_server()
```
