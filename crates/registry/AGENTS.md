# Xeno Registry — AGENTS

This file is for contributors working on the `xeno-registry` crate. It describes the registry’s architecture, the data flow from authored metadata to runtime entries, and the conventions for adding or modifying domains.

The registry is **format-neutral at runtime**. KDL (currently) is **build-time authoring only**.

---

## Architecture overview

The registry is a typed database of editor definitions (actions, commands, options, themes, etc.) with:

- **Immutable snapshots** for readers (`RegistryRef` pins an `Arc<Snapshot>`).
- **Runtime registration** via **snapshot rebuild + CAS publish** (atomic swap).
- **Build-time compilation** of authored metadata into embedded binary specs.
- **Domain-colocated loaders/linkers** (each domain owns its spec → linked defs wiring).

### High-level components

- **Domain modules** (e.g. `src/actions/`, `src/options/`, `src/languages/`):
  - `spec` types (serde/postcard contract)
  - `loader` (decode embedded blob)
  - `link` (link spec ↔ Rust handler statics where applicable)
  - runtime `Entry` types and `Input` types ingested by the builder

- **Shared substrate**:
  - `src/defs/loader.rs`: blob header/validation + postcard decode
  - `src/defs/link.rs`: shared linking utilities (e.g. `link_by_name`)
  - `src/core/index/*`: generic index building, lookup resolution, collision recording, runtime registration

- **DB builder**:
  - `src/db/domain.rs`: `DomainSpec` trait (domain type plumbing + conversions + hooks)
  - `src/db/domains.rs`: domain marker types implementing `DomainSpec`
  - `src/db/builder/mod.rs`: `RegistryDbBuilder` and domain registration wrappers

### Pipeline (end-to-end)

**Build time**
1. Authoring input: domain metadata authored as KDL (currently) under repo assets.
2. Compilation: `crates/registry/build/*` parse the authoring format into domain spec structs.
3. Serialization: specs are serialized with `postcard` and written as `<domain>.bin` using the shared blob wrapper.
4. Embedding: `.bin` files are embedded into the final binary (`include_bytes!(concat!(env!("OUT_DIR"), ...))`).

**Runtime**
1. Load spec: domain module decodes embedded blob via `defs::loader`.
2. Link: domain module links spec metadata to Rust handler statics where applicable.
3. Build indices: `RegistryDbBuilder` ingests static + linked inputs, interns strings, assigns dense IDs, builds immutable `RegistryIndex` values.
4. Publish: each domain is exposed via a `RuntimeRegistry` holding an atomic `Snapshot`.
5. Extend: runtime registration builds a new snapshot and publishes it with CAS.

---

## Incremental updates (runtime registration)

Runtime registration is snapshot-based, but **lookup maintenance is incremental**:

- **Append** (new canonical ID): **O(1)** relative to registry size (work proportional to the new entry’s key set).
- **Replace** (canonical ID collision where incoming wins): **O(N × affected_keys)**, where `affected_keys` is the union of keys of the old + new entry (typically small). Only those keys are rescanned across entries.

This avoids rebuilding the entire lookup map for the common case (append), and bounds replace cost to a small number of targeted rescans.

### `by_id` map (Stage A only)

Snapshots and indices maintain two maps:

- `by_id`: **Canonical ID → Id** (Stage A only). Used for correct, O(1) canonical collision detection.
- `by_key`: **Any lookup key → Id** (Stages A/B/C as appropriate for lookup), used for general key resolution.

**Important:** `by_key` may contain secondary keys; **canonical-ID collision detection must use `by_id`**, not `by_key`.

---

## Build pipeline safety (`BuildCtx` + contracts)

### `BuildCtx`

`BuildEntry::build` no longer takes a raw interner; it takes:

- `fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> Entry`

All interning / symbol lookup performed during build must go through `BuildCtx`.

### String declaration contract

All strings used during `build()` must be declared ahead of time:

- Domains implement `collect_payload_strings` (payload-only)
- Core automatically collects **all metadata strings** (`id`, `name`, `description`, `short_desc`, and key strings) before build
- The builder interns the collected string set, then calls `build()`

### Enforcement

The “collect before use” contract is enforced:

- **in debug builds** (default)
- **in any build** when enabled via feature: `registry-contracts`

If a `build()` attempts to intern/lookup a string not declared via collection, it panics with a precise message (string value included).

---

## Ergonomics helpers

### `StringCollector`

Domains use `StringCollector` for declarative payload string gathering:

- `push(&str)`
- `opt(Option<&str>)`
- `extend(iter of &str)`

Domains no longer manually collect meta strings; they only collect payload strings.

### `BuildCtxExt`

Extension methods reduce boilerplate in `build()` implementations:

- `intern_req(&str, what)`
- `intern_opt(Option<&str>)`
- `intern_iter(iter of &str)` (and similar helpers)

Use these consistently to keep domain build code compact and uniform.

---

## Linking utilities (`link_by_name` aggregate reporting)

For handler-driven domains (actions/commands/motions/etc.), linking is strict and name-based.

`defs::link::link_by_name` now performs a full scan before failing and produces a **single aggregate report** covering all issues:

- duplicate handlers
- duplicate spec/meta entries
- spec entries missing handlers
- handlers missing spec entries

This is intentionally fatal (panic), but dramatically improves debuggability when evolving specs/handlers.

---

## Glossary

### Registry lifecycle terms

- **Def**: Rust-side definition shape (builtins). May include function pointers / static tables.
- **Spec**: Format-neutral, data-only serialized contract embedded in the binary (postcard blob).
- **LinkedDef**: A runtime input produced by linking spec metadata with Rust handler statics.
- **Input**: Domain-specific value ingested by the builder (often `Static(...)` or `Linked(...)`).
- **Entry**: Runtime, symbolized form stored in the registry index.

### Identity and lookup

- **Canonical ID (Stage A)**: Stable identifier (often `"xeno-registry::<name>"`). Collision detection uses `by_id`.
- **Primary Name (Stage B)**: Human-facing name used for display and lookup.
- **Secondary Keys (Stage C)**: Additional lookup strings (aliases, config keys, etc.).
- **Resolution**: The 3-stage lookup contract. Stage precedence is enforced consistently across build and runtime registration.

### Core types

- **`RegistryIndex<T, Id>`**: Immutable, indexed storage produced by the builder (tables, maps, interner, collisions).
- **`Snapshot<T, Id>`**: Published runtime view (Arc-pinned); readers hold these alive via `RegistryRef`.
- **`RegistryRef<T, Id>`**: Pinned handle to an entry; keeps its source snapshot alive.
- **`RuntimeRegistry<T, Id>`**: Atomic container for current snapshot; supports registration via snapshot rebuild + CAS.
- **`Party`**: Comparable metadata used to resolve conflicts deterministically (priority, source rank, ordinal).
- **`Collision`**: Diagnostic record for key conflicts; ordering is deterministic via `Collision::stable_cmp`.

---

## Invariants and mental model

### Snapshot semantics

- Readers load the current snapshot and use it for all lookups.
- `RegistryRef` pins the snapshot it came from; it never observes partial updates.

### Dense IDs

Domains assign dense IDs (`ActionId`, `OptionId`, etc.) during build. IDs index compact tables for O(1) access.

### Precedence (conflict resolution)

Resolution follows a strict precedence hierarchy (applies at build and runtime):

1. Higher `priority` wins.
2. Higher `source` rank wins (Runtime > Crate > Builtin).
3. Deterministic tie-breakers via `Party` ordering.

### Determinism

- Collision lists are sorted with `Collision::stable_cmp`.
- Stable ordering must not depend on incidental insertion order.

---

## `DomainSpec` and domain wiring

The `define_domains!` macro is intentionally minimal. All domain plumbing lives in `DomainSpec` implementations.

### `DomainSpec` responsibilities

Each domain marker type implements `DomainSpec`:

- associated types: `Input`, `Entry`, `Id`, `StaticDef`, `LinkedDef`
- conversions: `static_to_input`, `linked_to_input`
- `LABEL`: human-readable domain label
- `builder(db) -> &mut RegistryBuilder<...>`: selects the correct builder field
- optional `on_push(db, &input)`: domain-specific side effects (e.g. collecting keybindings)

This keeps domain behavior explicit and local, while keeping `RegistryDbBuilder` boilerplate flat.

---

## Adding a new domain

When adding a new domain, treat it as three layers:

1) Domain runtime types (`Entry`/`Input`)
2) Spec + loader + link (format-neutral runtime pipeline)
3) DB wiring (`DomainSpec` + builder registration)

### 1) Define domain runtime types

Create a new module (typically `src/<domain>/`):

- `entry.rs`: define `<Domain>Entry` implementing `RegistryEntry`
- `def.rs`: define `<Domain>Def` and `<Domain>Input` (often `Static` + `Linked`)
- `link.rs`: if handler-driven, define handler statics and linking rules
- `loader.rs`: load embedded `<domain>.bin`
- `spec.rs` (or `types.rs`): serde contract used by build + runtime decode

### 2) Implement build contract (`BuildEntry`)

Your domain `Input` type must implement `BuildEntry<Entry>`:

- implement `collect_payload_strings(&self, collector: &mut StringCollector<...>)`
  - payload-only strings
  - do **not** collect meta strings (core does this automatically)
- implement `build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> Entry`
  - use `BuildCtxExt` helpers (`intern_req`, `intern_opt`, `intern_iter`)
  - never access interners directly

Run with `--features registry-contracts` if you’re touching collection/build logic.

### 3) Add the compiled spec pipeline (format-neutral)

**Runtime (domain module)**:
- define `Spec` contract structs (serde/postcard)
- `loader.rs`: decode embedded blob using `defs::loader`
- `link.rs`: domain-specific linking/validation/parsing
  - handler-driven domains should use `defs::link::link_by_name`

**Build time (`crates/registry/build/`)**:
- parse authoring format (KDL)
- emit the same `Spec` structs
- serialize via `postcard` using the shared blob wrapper
- emit `cargo:rerun-if-changed` for authoring inputs

### 4) Wire into `RegistryDbBuilder`

1. Add a domain marker type implementing `DomainSpec` in `src/db/domains.rs`.
   - include conversions (`static_to_input`, `linked_to_input`)
   - add `on_push` if you need side tables (e.g. keybindings)

2. Add one line to the `define_domains!` invocation in `src/db/builder/mod.rs`:
   - `field` name (builder field)
   - `stem` (register method prefix)
   - `domain` marker type

That’s it. The macro should not grow knobs.

### 5) Tests (required)

- Add a domain-level consistency test:
  - handler-driven: spec ↔ handler bijection
  - spec-only: spec validity and linking rules
- Add targeted tests for parsing/validation rules if the domain has them.
- Run the registry feature matrix:
  - `cargo test -p xeno-registry`
  - `cargo test -p xeno-registry --no-default-features`
  - `cargo test -p xeno-registry --all-features`
  - `cargo test -p xeno-registry --features registry-contracts`
  - `cargo clippy -p xeno-registry --all-targets --all-features`

---

## Supporting a new authoring format

Runtime is format-neutral. Adding a new authoring format means:

- implement a new build-time compiler producing the existing `Spec` structs
- emit the same `<domain>.bin` blobs

No runtime changes should be required.

---

## Practical contributor notes

- Keep parsing/validation in the **domain linker** (`<domain>/link.rs`) unless it is purely authoring-syntax-specific.
- Prefer `BuildCtxExt` + `StringCollector` over ad-hoc interning/collection.
- If you touch shared spec types, update build-time copies and verify postcard compatibility.
- If you touch linking, ensure failures remain aggregate and deterministic (`link_by_name` report, stable collision ordering).
