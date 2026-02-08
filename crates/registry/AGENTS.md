# Xeno Registry

The Xeno registry is a concurrent, runtime-extensible database for editor definitions - actions, commands, motions, options, and other declarative items that must be searchable by human-friendly keys while remaining stable and cheap to access at runtime.

At a high level, the registry separates _construction_ (interning, deduplication, indexing) from _consumption_ (wait-free reads) and supports extension (atomic publication of new snapshots) without invalidating existing references.

## Architecture overview

A registry domain begins life as a set of static definitions (`Def`) authored as `const`/`static` values using raw strings and other `'static` references. During startup, these definitions are ingested by `RegistryBuilder`, which:

- interns all strings into a compact symbol table,
- assigns dense IDs for O(1) table indexing,
- canonicalizes duplicate canonical IDs according to the configured policy, and
- produces an immutable `RegistryIndex` (the built form of the domain).

The `RegistryIndex` is then wrapped by `RuntimeRegistry`, which publishes the current state as an atomically-swappable `Snapshot`. Reads load a snapshot and perform lookups without locking. Runtime extension follows the same pipeline: build a new snapshot (including a rebuilt key map) and atomically publish it.

This yields three important properties:

- lookups are O(1) and require only an atomic load plus map/table access.
- returned handles pin the snapshot they came from.
- plugins can register new definitions without coordinating with readers.

## Glossary

| Term | Definition |
|------|------------|
| Canonical ID | Stage A key. Immutable unique identifier for a definition (e.g., `xeno-registry::quit`). |
| Primary Name | Stage B key. Friendly display name used for lookup (e.g., `quit`). |
| Secondary Keys | Stage C keys. User-defined aliases and domain-specific lookup keys (e.g., `q` for `quit`). |
| Symbol | Interned string identifier (`u32`). Fast to compare and copy. |
| Key Pool | A shared pool of `Symbol`s within a `Snapshot` used to store secondary keys. |
| Snapshot | Immutable point-in-time view of a registry domain. |
| Pinning | Mechanism where a `RegistryRef` holds an `Arc<Snapshot>`, keeping the data alive even if a new snapshot is published. |
| DomainSpec | Trait defining the contract for a registry domain (input, entry, and builder wiring). |
| DefInput | Unified wrapper for static (macro-authored) and linked (dynamic) definitions. |

## Snapshots and read semantics

A `Snapshot` is the single source of truth for lookups. It contains:

- a dense table of entries (`table`),
- a key map (`by_key`) from interned symbols to dense IDs,
- a frozen interner used to resolve symbols back to strings,
- a key pool (`key_pool`) of interned symbols, and
- diagnostic collision records.

Lookups return a `RegistryRef`, which holds an `Arc<Snapshot>` and a dense ID. This is the pinning mechanism: once a `RegistryRef` exists, the snapshot it refers to cannot be freed even if a newer snapshot is published.

## Precedence and conflict resolution

The registry resolves overlaps deterministically and records the losing parties for diagnostics.

When two definitions claim the same canonical ID, the registry selects exactly one winner for that ID. The precedence rule is:

1. higher priority wins,
2. higher source rank wins (`Runtime > Crate > Builtin`),
3. later ingest ordinal wins (a deterministic tie-break).


Secondary keys (Primary Name + Secondary Keys) are bindings into the key map. When multiple definitions compete for the same non-canonical key, the precedence rule is:

1. higher priority wins,
2. higher source rank wins (`Runtime > Crate > Builtin`),
3. canonical ID symbol order breaks ties (stable, deterministic).

This is the rule used for 'which definition a user gets when they type a key.'

Once a canonical ID is bound, it cannot be displaced by a secondary key. Keys compete only within their stages; they do not override canonical identity bindings.

## Duplicate policy

Duplicate canonical IDs are governed by `DuplicatePolicy` (e.g. `ByPriority`, `FirstWins`, `LastWins`, `Panic`). In production builds the typical configuration is deterministic:

- duplicates resolve predictably,
- losers are excluded from effective lookup tables, and
- collisions remain available for diagnostics.

In dev, `Panic` can be used to force early discovery of bad domain composition.

## Adding a new domain

A domain is a small, regular structure defined by a `DomainSpec` implementation.

1. Create `src/<domain>/` with the following shape:
   - `def.rs` — `<Domain>Def`: the static definition type (`'static` authoring surface).
   - `entry.rs` — `<Domain>Entry`: the runtime storage type used in tables and returned from lookups.
   - `builtins.rs` — built-in definition set and `register_builtins` function.
   - `mod.rs` — public facade, `Input` type alias (via `DefInput`), and re-exports.

2. Implement `DomainSpec` for the new domain in `crates/registry/src/db/domains.rs` using the `domain!` macro (or manually if custom `on_push` logic is needed).

3. Wire the domain into `crates/registry/src/db/mod.rs` as part of the `RegistryDb` struct and `ACTIONS`/`OPTIONS` style static accessors.

For a canonical example, see `src/options/`.

### Recipe: Adding a new key to an existing domain

If you add a new lookup field (like `kdl_key`) to a domain:
1. Update `collect_strings` in `def.rs` to call `collect_meta_strings(..., [extra_key])`.
2. Update `build` in `def.rs` to pass the extra key(s) to `build_meta(..., [extra_key])`.
3. The new key will automatically join **Stage C** (Secondary Keys) and cannot displace Stage A or Stage B bindings.

## Invariants

The implementation is organized around a few non-negotiable invariants:

- exactly one winner per canonical ID.
- stable ordering by canonical ID (and stable tie-breaking for conflicts).
- any `RegistryRef` keeps its source snapshot alive.
- concurrent registrations are atomic and do not lose updates.

These invariants are enforced both structurally (immutability + snapshot pinning) and by tests in the index invariant suite.

## Debugging collisions

When a definition loses a canonical-ID or key conflict, the registry records a `Collision` in the current snapshot. This is intended for “why didn’t my plugin override X?” questions.

Example: If a plugin tries to bind `:q` to its own action but the built-in `quit` action already has it as an alias, the snapshot will record a `KeyConflict`.
- key: `:q` (symbol)
- existing: `quit` (Party { source: Builtin, ... })
- incoming: `my-plugin-quit` (Party { source: Runtime, ... })
- resolution: `ReplacedExisting` (since Runtime > Builtin)

To inspect:

- read `Snapshot::collisions` from the current snapshot, or
- use the editor’s registry diagnostics command (e.g. `:registry collisions`) if available in your integration layer.

Collision records include the conflicting key, the parties involved, and the resolution that was applied.
