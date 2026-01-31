# Registry System

## Purpose
- Owns: Subsystem definition indexing, runtime extension registration, and collision/winner resolution.
- Does not own: Command execution, hook emission (owns the definitions only).
- Source of truth: `RegistryDb` and its domain-specific `RuntimeRegistry` instances (storing `Snapshot<T>`).

## Mental model
- Terms: Builtin (compile-time), Snapshot (atomic view), Winner (conflict resolution), Eviction (cleanup after override).
- Lifecycle in one sentence: A build-time index is wrapped in an `ArcSwap` snapshot, allowing atomic, lock-free runtime extension with deterministic collision rules.

## Module map
- `core::index` — Generic indexing, insertion, and collision logic.
- `db` — Database root and builtin/plugin orchestration.
- `<domain>::registry` — Domain-specific wrappers (Actions, Options, etc.).

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| `Snapshot<T>` | Atomic view of registry | Immutable, lock-free read | `RuntimeRegistry` |
| `KeyStore` | Abstract index mutation | MUST implement `evict_def` | `SnapshotStore` / `BuildStore` |
| `DuplicatePolicy` | Conflict resolution rule | MUST be deterministic | `RuntimeRegistry::with_policy` |
| `InsertAction` | Outcome of registration | Informs diagnostics | `insert_typed_key` |
| `KeyKind` | Type of registration key | Id, Name, or Alias | `RegistryMeta` |

## Invariants (hard rules)
1. MUST keep ID lookup unambiguous (one winner per ID).
   - Enforced in: `insert_typed_key` (build-time), `insert_id_key_runtime` (runtime).
   - Tested by: `core::index::tests::test_id_first_lookup`
   - Failure symptom: Panics during build/startup or stale name lookups after override.
2. MUST evict old definitions on ID override.
   - Enforced in: `insert_id_key_runtime` (calls `evict_def`).
   - Tested by: `core::index::tests::test_id_override_eviction`
   - Failure symptom: Stale name/alias lookups pointing to a replaced definition.
3. MUST maintain stable numeric IDs for builtin actions.
   - Enforced in: `RegistryDbBuilder::build`.
   - Tested by: TODO (add regression: test_stable_action_ids)
   - Failure symptom: Inconsistent `ActionId` mappings in optimized input handling.

## Data flow
1. Builtins: `inventory` or explicit registration builds base index via `RegistryBuilder`.
2. Plugins: Sorted by priority, executed to mutate the builder.
3. Snapshot: `RuntimeRegistry` loads built index into a `Snapshot`.
4. Mutation: `register` clones snapshot, applies changes via `insert_id_key_runtime`, and CAS (Compare-and-Swap) updates.
5. Resolution: `get(key)` checks ID table then Key table in O(1).

## Lifecycle
- Build Phase: Builtins registered; plugins run; index finalized.
- Runtime Phase: snapshots loaded; runtime registration allowed; lookups lock-free.

## Concurrency & ordering
- Lock-free reads: `snap.load()` provides a stable pointer without blocking.
- CAS Retry Loop: Writes retry if the snapshot changed during mutation.
- Deterministic Winners: `DuplicatePolicy` ensures the same definition wins regardless of registration order.

## Failure modes & recovery
- Duplicate Build ID: Panic during startup to prevent ambiguous behavior.
- CAS Contention: Writes may take multiple retries under heavy concurrent registration.
- Eviction Failure: If not implemented for a new index kind, stale definitions persist.

## Recipes
### Override a builtin definition
Steps:
- Call `registry.try_register_override(new_def)`.
- Ensure `new_def.meta().priority` is higher than the existing one if using `ByPriority`.

## Tests
- `core::index::tests::test_id_override_eviction`
- `core::index::tests::test_register_many_atomic_failure`
- `core::index::tests::test_total_order_tie_breaker`

## Glossary
- Builtin: A definition registered at compile-time.
- Snapshot: A point-in-time view of all registered definitions.
- Winner: The definition that survives a collision based on policy.
- Eviction: The process of removing all keys pointing to a replaced definition.
