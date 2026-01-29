# Xeno Registry System

This guide describes the current registry architecture: build-time indices, runtime mutation via ArcSwap snapshots, explicit ID override APIs, and domain registries with derived indices kept inside the snapshot.

Key files (read these first):
- Runtime snapshot core: `crates/registry/src/core/index/runtime.rs`
- Domain registries with derived indices:
  - `crates/registry/src/textobj/registry.rs`
  - `crates/registry/src/options/registry.rs`
  - `crates/registry/src/hooks/registry.rs`
- Database init and public surfaces: `crates/registry/src/db/mod.rs`
- Plugin execution ordering: `crates/registry/src/db/plugin.rs`
- Plugin definition type: `crates/registry/src/core/plugin.rs`

Scope: actions, commands, motions, themes, gutters, statusline, text objects, options, hooks, notifications.

## Mental Model

- A registry item is a `&'static T` where `T: RegistryEntry`.
- Build phase produces `RegistryIndex<T>` per domain (static/builtin content).
- Runtime phase wraps the built index in a registry that owns an ArcSwap snapshot (single atomic source of truth for lookups).
- Lookups are read-only and lock-free: `snap.load()` and map lookups.
- Writes are atomic batches: clone snapshot → mutate → compare_and_swap (CAS) retry loop.

The result is that builtins and runtime extensions are always viewed as a single coherent snapshot, not layered maps.

## Data Model and Invariants

RegistryEntry / metadata:
- `T: RegistryEntry` exposes `meta() -> &RegistryMeta`, including:
  - `id`, `name`, `aliases`, `priority`, `source`, required caps, etc.

Invariants to preserve:
- IDs are the stable identity. Treat ID uniqueness as a rule for additive registration.
- Name/alias are user-facing keys. Collisions are allowed, but must be deterministic (`DuplicatePolicy`).
- Runtime registration is load-only (definitions are leaked to `'static` via `Box::leak` or statics).
- `ActionId` mapping is startup-only and ignores runtime registration. `ActionId` provides a dense numeric ID over builtin actions for performance.

## Build Flow (Builtins + Plugins)

`RegistryDb` is constructed once via `get_db()` in `crates/registry/src/db/mod.rs`:
1. Register builtins explicitly.
2. Run plugins (sorted by priority).
3. Build numeric ID mapping for actions (builtin actions only).
4. Construct `RuntimeRegistry` instances with the resulting indices.

## Runtime Registration

Registries support runtime extension via `register` and `register_owned` (which leaks the definition to `'static`).

ID Override Support:
- `try_register_override` allows replacing an existing definition with the same ID if the `DuplicatePolicy` (e.g., `ByPriority`) chooses the new one.
- Shared logic for ID overrides is centralized in `insert_id_key_runtime` to ensure consistent behavior across all domain registries.

## Domain-Specific Registries

Some domains require extra indices:
- `TextObjectRegistry`: indexed by trigger character.
- `OptionsRegistry`: indexed by KDL key string.
- `HooksRegistry`: indexed by event type.

These extra indices are managed within the snapshot and updated atomically during registration.

## Performance Notes

- Lookups by ID, name, or alias are O(1) via hash maps.
- `resolve_action_id_from_static` is a linear scan O(n) over the built action list. It should be used sparingly if the action list is large.
- Numeric `ActionId` lookups are O(1) via array indexing.
