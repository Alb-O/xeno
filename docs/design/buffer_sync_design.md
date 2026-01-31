# Cross-process document synchronization via broker

## 1. Purpose

Provide real-time text convergence for the same file opened in multiple **independent Xeno editor processes** by routing edit deltas through the broker. Primary UX: type in one terminal, other terminal(s) update immediately.

Non-goals (v1):
- true multi-writer collaborative editing (CRDT/OT merge)
- cross-host sync
- persistence of unsaved shared state across broker restart

## 2. Mental model

- A *Sync Doc* is keyed by canonical `uri` (use the same URI you already use for LSP: `file://…`).
- The broker maintains an **authoritative Sync Doc state** for each open URI: `(epoch, seq, rope)`.
- Exactly one session is the **doc owner** (single-writer) at any moment (reuse the existing single-writer concept from `gate_text_sync`).
- Owner publishes `Transaction` deltas to broker; broker validates + applies to its rope; broker broadcasts the delta to all follower sessions.
- Followers are **live views**: they apply remote deltas locally (no undo recording), and map cursor/selection through the transaction (same mechanism as `sync_sibling_selections` / `map_selection_through`).

Conflict resolution is therefore **ownership arbitration**, not textual merge:
- If a non-owner attempts to edit, broker rejects (`NotDocOwner`) and editor should enter read-only mode for that doc, with a “take ownership” affordance.

## 3. Module map

Broker side:
- `crates/broker/broker-proto/src/types.rs`
  - add wire types for buffer sync (requests/responses/events)
- `crates/broker/broker/src/core/mod.rs`
  - extend `BrokerCore` to include `sync_docs: HashMap<String, SyncDocState>`
  - extend `gate_text_sync` to also cover `BufferSync*` requests
  - implement `on_buffer_delta` and `apply_and_broadcast_delta`
- `crates/broker/broker/src/service.rs`
  - route new `RequestPayload::*` to `BrokerCore` methods

Editor side:
- `crates/editor/src/lsp/broker_transport.rs`
  - extend `BrokerClientService::notify` to handle `Event::BufferSync*` and forward to editor core
- `crates/editor/src/buffer_sync/mod.rs` (new)
  - `BufferSyncManager`: tracks per-open-doc `{ uri, epoch, seq, role }`
- `crates/editor/src/impls/undo_host.rs`
  - hook in `EditorUndoHost::apply_transaction_inner` after `result.applied` to emit sync delta
  - utilize `sync_sibling_selections` (line 141) as a base for remote edit mapping
- `crates/editor/src/view_manager.rs`
  - used for mapping URIs to all local `ViewId`s when applying remote deltas
- `crates/editor/src/buffer/mod.rs`
  - `Buffer::map_selection_through` (line 412) is the core mechanism for cursor sync
- `crates/editor/src/buffer/document/mod.rs`
  - `Document` versioning and `commit_unchecked` for non-undoable remote updates

## 4. Key types

| Type | Location | Description |
|---|---|---|
| `SyncEpoch(u64)` | broker-proto | Monotonic “ownership generation” for a URI. Increments on ownership change. |
| `SyncSeq(u64)` | broker-proto | Monotonic edit sequence per URI under an epoch. Strictly increments per applied delta. |
| `WireOp` | broker-proto | Serializable op: `Retain{n}`, `Delete{n}`, `Insert{utf8}`. |
| `WireTx` | broker-proto | Serializable transaction: `Vec<WireOp>`. |
| `BufferSyncRole` | broker-proto | `Owner` / `Follower`. |
| `SyncDocState` | broker | `{ owner, open_refcounts, epoch, seq, rope, last_hash? }` |
| `BufferSyncManager` | editor | Tracks per-open-doc `{ uri, epoch, seq, role }`, emits outgoing deltas, applies incoming. |

## 5. Invariants (hard rules)

1. **Single-writer gating**
- Rule: Only `SyncDocState.owner` MAY submit deltas for a URI/epoch.
- Enforced in: `crates/broker/broker/src/core/mod.rs::BrokerCore::on_buffer_delta`
- Tested by: `crates/broker/broker/src/core/tests::test_buffer_sync_rejects_non_owner`
- Failure symptom: split-brain divergence; followers see different text vs owner.

2. **Seq monotonicity**
- Rule: Broker MUST accept deltas only if `(delta.epoch == state.epoch && delta.base_seq == state.seq)`.
- Enforced in: `BrokerCore::on_buffer_delta`
- Tested by: `test_buffer_sync_seq_mismatch_triggers_resync`
- Failure symptom: follower applies out-of-order ops → panic/incorrect rope indices.

3. **Broker rope is authoritative**
- Rule: Broker MUST apply the delta to its rope before broadcasting, and broadcast the same delta it applied (plus the new `seq`).
- Enforced in: `BrokerCore::apply_and_broadcast_delta`
- Tested by: `test_buffer_sync_broadcast_matches_broker_rope`
- Failure symptom: broker and followers drift; resync loops.

4. **Follower remote apply is non-undoable**
- Rule: Remote deltas MUST NOT create local undo steps in follower sessions.
- Enforced in: `crates/editor/src/buffer_sync/mod.rs::BufferSyncManager::apply_remote_delta` using `UndoPolicy::NoUndo`
- Tested by: `crates/editor/...::test_remote_apply_does_not_affect_undo_stack`
- Failure symptom: undo in follower removes remote edits unexpectedly.

5. **Selections always mapped through any applied delta**
- Rule: After applying any delta (local or remote), all views of that document MUST clamp + remain valid.
- Enforced in: editor: `BufferSyncManager::map_all_views_for_doc_through(tx)` + `ensure_valid_selection`
- Tested by: `test_remote_edit_maps_selection_across_views`
- Failure symptom: cursor jumps OOB, rendering panics, selection corruption.

## 6. Data flow

### Open
1. Editor opens file → computes `uri` and loads local content.
2. Editor sends `RequestPayload::BufferSyncOpen { uri, text, version_hint }`.
3. Broker:
   - if first open: installs rope from `text`, sets owner=session, epoch+=1, seq=0, role=Owner
   - else: increments refcount, role=Follower, returns snapshot `{ text, epoch, seq }` so follower converges immediately
4. Editor:
   - if follower and snapshot differs: replace doc content wholesale (NoUndo), map selections, mark “sync follower” (read-only edits)

### Edit (owner)
1. Local edit applies in editor (`Transaction` already built).
2. Editor serializes tx → `WireTx`, sends `RequestPayload::BufferSyncDelta { uri, epoch, base_seq, tx }`.
3. Broker validates (owner + seq), applies to broker rope, increments seq, broadcasts `Event::BufferSyncDeltaApplied { uri, epoch, seq, tx }`.
4. Followers receive and apply tx locally, update their `(epoch, seq)`.

### Ownership transfer
- Explicit: editor sends `BufferSyncTakeOwnership { uri }`.
- Implicit: if owner session disconnects or closes doc, broker re-elects owner as `min(session_id)` among open sessions.
- Broker increments `epoch`, resets `seq=0` (or keep seq but epoch change makes ordering unambiguous), broadcasts `Event::BufferSyncOwnerChanged { uri, epoch, owner }`.
- Editors update role. New owner becomes editable.

### Resync (desync or reconnect)
- If editor sees `(epoch mismatch) or (seq gap)`, it sends `BufferSyncResync { uri }`.
- Broker responds with full snapshot `{ text, epoch, seq, owner }`.

## 7. Lifecycle

Per-editor session:
- connect broker transport → `Subscribe`
- on doc open → `BufferSyncOpen`
- on doc close → `BufferSyncClose` (decrement refcount; may trigger owner re-election)
- on edit:
  - if role=Owner: send delta
  - if role=Follower: block edit locally OR attempt ownership acquisition then re-run edit (policy decision)

Per-broker Sync Doc:
- created on first `BufferSyncOpen`
- destroyed when last refcount hits 0 (drop rope to free memory)

## 8. Concurrency & ordering

Broker:
- all sync doc mutations under `BrokerCore.state` mutex; seq/epoch checks happen under lock
- broadcast occurs outside lock but after state mutation is committed

Editor:
- incoming broker events arrive on broker transport task; must be routed onto editor’s main loop (same pathway as other broker events)
- apply remote delta must be serialized with local edits. Because follower edits are disallowed, “real” races are ownership change edges:
  - on `OwnerChanged`, set a per-doc `epoch` immediately; reject/ignore any stale delta for old epoch.

## 9. Failure modes & recovery

- Broker restart / disconnect:
  - editor transport reports `Disconnected`; BufferSyncManager marks all docs `SyncDisabled` and restores local editing.
- Seq mismatch (missed deltas):
  - follower requests `Resync` and installs snapshot.
- Non-owner edit attempt:
  - broker rejects `NotDocOwner`; editor shows toast and either:
    - keep follower read-only, or
    - auto “take ownership” and retry the edit (risky UX; optional)
- Huge snapshot:
  - enforce max snapshot bytes (e.g. 8–32 MiB) to avoid IPC blowups; if exceeded, fall back to “sync disabled for this doc”.

## 10. Recipes

- “View-only follower”: open same file in second terminal; it will converge and follow live edits.
- “Take ownership”: run command `:sync-take-ownership` → broker flips owner; other session becomes follower and becomes read-only.
- “Force resync”: `:sync-resync` → request broker snapshot.

## 11. Tests

Broker (`crates/broker/broker/src/core/tests.rs`):
- `test_buffer_sync_open_owner_then_follower_gets_snapshot`
- `test_buffer_sync_rejects_non_owner`
- `test_buffer_sync_seq_mismatch_triggers_resync`
- `test_buffer_sync_owner_disconnect_elects_successor_epoch_bumps`

Editor (new tests near BufferSyncManager):
- `test_remote_apply_does_not_affect_undo_stack`
- `test_remote_edit_maps_selection_across_views`
- `test_epoch_mismatch_ignores_delta_and_requests_resync` (if you implement auto-resync)

E2E (optional):
- Simulate two sessions using `PeerSocket` pairs; open doc; send delta; verify both local ropes equal broker rope.

## 12. Glossary

- **URI**: canonical document key (prefer LSP’s `file://` URI to avoid path normalization bugs).
- **Owner**: the single writer session for a URI; only source of deltas.
- **Follower**: live-view session that applies remote deltas but cannot author them.
- **Epoch**: generation counter for ownership; prevents stale deltas after ownership change.
- **Seq**: per-epoch edit counter; enforces total order of applied deltas.
- **Snapshot**: full UTF-8 text of current rope used to initialize or resync a follower.
