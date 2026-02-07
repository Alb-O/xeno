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

## Snapshots and read semantics

A `Snapshot` is the single source of truth for lookups. It contains:

- a dense table of entries (`table`),
- a key map (`by_key`) from interned symbols to dense IDs,
- a frozen interner used to resolve symbols back to strings, and
- diagnostic collision records.

Lookups return a `RegistryRef`, which holds an `Arc<Snapshot>` and a dense ID. This is the pinning mechanism: once a `RegistryRef` exists, the snapshot it refers to cannot be freed even if a newer snapshot is published.

## Precedence and conflict resolution

The registry resolves overlaps deterministically and records the losing parties for diagnostics.

When two definitions claim the same canonical ID, the registry selects exactly one winner for that ID. The precedence rule is:

1. higher priority wins,
2. higher source rank wins (`Runtime > Crate > Builtin`),
3. later ingest ordinal wins (a deterministic tie-break).


Names and aliases are bindings into the key map. When multiple definitions compete for the same non-canonical key, the precedence rule is:

1. higher priority wins,
2. higher source rank wins (`Runtime > Crate > Builtin`),
3. canonical ID symbol order breaks ties (stable, deterministic).

This is the rule used for 'which definition a user gets when they type a key.'

Once a canonical ID is bound, it cannot be displaced by a name or alias. Names/aliases compete only within their stages; they do not override canonical identity bindings.

## Duplicate policy

Duplicate canonical IDs are governed by `DuplicatePolicy` (e.g. `ByPriority`, `FirstWins`, `LastWins`, `Panic`). In production builds the typical configuration is deterministic:

- duplicates resolve predictably,
- losers are excluded from effective lookup tables, and
- collisions remain available for diagnostics.

In dev, `Panic` can be used to force early discovery of bad domain composition.

## Adding a new domain

A domain is a small, regular module structure plus a registration hook into the database builder.

Create `src/<domain>/` with the following shape:

- `def.rs` — `<Domain>Def`: the static definition type (`'static` authoring surface) and `BuildEntry` implementation.
- `entry.rs` — `<Domain>Entry`: the runtime storage type used in tables and returned from lookups.
- `builtins.rs` — built-in definition set and `register_builtins` function.
- `mod.rs` — public facade and re-exports.

Then wire the domain into:

- `crates/registry/src/db/mod.rs` (domain is part of the DB surface), and
- the DB builder module (domain is built and published during startup).

For a canonical example, see `src/options/`.

## Invariants

The implementation is organized around a few non-negotiable invariants:

- exactly one winner per canonical ID.
- stable ordering by canonical ID (and stable tie-breaking for conflicts).
- any `RegistryRef` keeps its source snapshot alive.
- concurrent registrations are atomic and do not lose updates.

These invariants are enforced both structurally (immutability + snapshot pinning) and by tests in the index invariant suite.

## Debugging collisions

When a definition loses a canonical-ID or key conflict, the registry records a `Collision` in the current snapshot. This is intended for “why didn’t my plugin override X?” questions.

To inspect:

- read `Snapshot::collisions` from the current snapshot, or
- use the editor’s registry diagnostics command (e.g. `:registry collisions`) if available in your integration layer.

Collision records include the conflicting key, the parties involved, and the resolution that was applied.
