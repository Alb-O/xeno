# Xeno Registry — AGENTS

This file is for contributors working on the `xeno-registry` crate. It describes the registry’s architecture, the data flow from authored metadata to runtime entries, and the conventions for adding or modifying domains.

The registry is format-neutral at runtime. KDL is only an authoring format at build time.

## Architecture overview

The registry is a typed, immutable index built at startup from a mix of:

1. Rust statics (builtins): handlers / definitions compiled into the binary.
2. Compiled specs (metadata): postcard-serialized binary blobs embedded in the binary and decoded at startup.

The runtime does not parse KDL (or any text format). Instead, the build scripts parse and emit compiled specs.

### Pipeline (end-to-end)

**Build time**
1. Authoring input: domain metadata authored as KDL (currently) under the repo’s data assets.
2. Compilation: `build.rs` + `build_src/*` parse the authoring format and produce a domain spec value (Rust struct).
3. Serialization: the spec is serialized using `postcard` and written to `<domain>.bin` using the shared blob wrapper format.
4. Embedding: the `.bin` files are embedded in the final binary via `include_bytes!(concat!(env!("OUT_DIR"), ...))`.

**Runtime**
1. Load spec: domain module decodes the embedded blob with `defs::loader`.
2. Link: domain module links spec metadata with Rust statics (handlers) where applicable.
3. Build index: `RegistryDbBuilder` ingests static + linked inputs, interns strings, assigns dense IDs, and builds immutable indices.
4. Runtime access: the resulting indices are exposed via typed refs and stable snapshot semantics (unchanged).

### Colocation

Each domain owns its own compilation boundary:

```
src/<domain>/
	spec.rs    // serde structs defining the format-neutral spec contract
	loader.rs  // loads embedded <domain>.bin via defs::loader
	link.rs    // domain-specific linking/validation/parsing (handlers, enums, etc.)
````

This replaces the old centralized `src/kdl/*` mirror.

### Shared substrate (`src/defs`)

`crates/registry/src/defs` is a small shared layer used by all domains:

- `defs/spec.rs`: shared spec types used across domains (e.g. `MetaCommonSpec`)
- `defs/loader.rs`:  shared blob header/validation and postcard decode (`load_blob`)
- `defs/link.rs`: shared linking utilities (`link_by_name`, `build_name_map`)

Domains should depend on `defs/*`, not on other domains’ internal loaders/linkers.

## Glossary

### Registry lifecycle terms

- `Def`: a Rust-side definition shape (builtins) that may include function pointers / static tables.
- `Input`: the type ingested by the builder for a domain (often `Static(def)` or `Linked(def)`).
- `Entry`: the runtime, symbolized, interned form stored in the immutable registry index.

### Identity and lookup

- Canonical ID: the stable identifier for an entry (usually `"xeno-registry::<name>"`). Canonical IDs are the primary keys for typed handles like `LookupKey<T, Id>`.
- Primary Name: human-facing short name (often equal to the suffix of the canonical ID); used for display and authoring.
- Secondary Keys: additional strings used for lookup (aliases, config keys like option `kdl_key`, etc.). These are attached to metadata and participate in resolution.
- Resolution: the 3-stage lookup contract used by the registry (canonical IDs / names / secondary keys), producing typed `RegistryRef`s.

### Linking terms

- Linking: pairing spec metadata with Rust statics (handlers) to produce `LinkedDef<Payload>` inputs.
- Bijection linking: for handler-driven domains (actions/commands/motions/etc.), linking is name-based and strict:
  - every spec item must have a handler
  - every handler must have a spec item
  (enforced by `defs::link::link_by_name`)

---

## Invariants and mental model

### Immutability (runtime)

The registry indices are immutable after initialization. Plugins and runtime extension do not mutate the registry indices (the “snapshot + CAS” design is not part of the runtime path here).

### Dense IDs

Domains assign dense IDs (e.g. `ActionId`, `CommandId`) during build. These IDs index into compact tables for O(1) access.

### Snapshot and resolution

Snapshot and resolution behavior is unchanged:
- `RegistryRef<T, Id>` points at a typed entry in the built index
- `LookupKey<T, Id>` allows lookup by canonical ID or `RegistryRef`
- the builder creates the final `RegistryIndex` tables used by runtime access

---

## Adding a new domain

When adding a new domain, treat it as three layers:

1. Domain types + entry
2. Spec + loader + linker
3. Builder wiring + tests

### 1) Define the domain runtime types

Create `src/<domain>/` with:
- `entry.rs`: define `<Domain>Entry` implementing `RegistryEntry`
- `def.rs`: define `<Domain>Def` and `<Domain>Input` (often `Static` + `Linked`)
- handler registry if needed (inventory + statics)

Also update:
- `crates/registry/src/db/domains.rs`: add a `DomainSpec` entry for the new domain
- `crates/registry/src/db/builder/mod.rs`: extend `define_domains!` to include the domain
- `crates/registry/src/lib.rs` and `src/<domain>/mod.rs` exports as appropriate

### 2) Add the compiled spec pipeline (format-neutral)

Inside `src/<domain>/`:

#### `spec.rs`
Define a serde `Spec` contract for the domain. Keep it data-only:
- include `common: MetaCommonSpec`
- represent enums as strings (e.g. `"buffer"`, `"global"`) unless you have a strong reason not to
- store parseable fields as strings if they originate as strings in authoring (width, position, etc.)

Example pattern:

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct DomainSpec {
  pub common: MetaCommonSpec,
  pub some_field: String,
  pub optional_field: Option<u64>,
}
```

#### `loader.rs`

Decode the embedded blob using the shared loader:

```rust
pub fn load_domain_spec() -> DomainSpecs {
  const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/domain.bin"));
  defs::loader::load_blob(BYTES, "<domain>")
}
```

#### `link.rs`

Implement the domain’s linking rules:

* If the domain is handler-driven (strict bijection):

  * iterate handler statics
  * use `defs::link::link_by_name` with the spec slice and handler slice
  * construct `LinkedDef<Payload>` where `Payload: LinkedPayload<Entry>`

* If the domain is spec-only (no handlers):

  * convert spec items directly into `LinkedDef` or into the appropriate `Input` variant
  * do domain-specific parsing/validation here (scope enums, numeric constraints, etc.)

Do not put generic linking logic in the domain; use `defs/link.rs`.

### 3) Build-time compiler

Add/extend build compiler under `crates/registry/build_src/`:

* Define build-time spec structs matching the runtime spec layout.
* Parse the authoring input (KDL) into the spec value.
* Serialize with `postcard` and write with the shared blob wrapper (`write_blob`).
* Emit `cargo:rerun-if-changed` for authoring inputs.

The build-time compiler should be the only place that knows about KDL syntax.

### 4) Builder wiring

In `db/builder/mod.rs`, add a `register_compiled_<domain>` method that:

* loads the spec via `<domain>::loader`
* links spec ↔ handlers via `<domain>::link` as needed
* registers linked defs into the domain builder
* registers any side tables (e.g. key prefix defs) if applicable

Then call this registration method from the domain’s builtin registration path (commonly `<domain>/builtins.rs`).

### 5) Tests (required)

Every new domain must add tests at two levels:

1. Generic substrate tests (already exist): `defs/tests.rs` should cover invariants of the shared linker and loader.
2. Domain consistency tests: add a test in `src/tests/consistency.rs` verifying spec↔handler consistency where applicable (bijection), or spec validity for spec-only domains.

If the domain includes parsing (enums, numeric widths, palette resolution, etc.), add at least one targeted test for those semantics.

## Supporting a new authoring format

Because runtime is format-neutral, adding a new authoring format means:

* implementing a build-time compiler that produces the same domain `Spec` structs
* emitting the same `<domain>.bin` blob(s)

No runtime changes should be required.

## Practical contributor notes

* Prefer keeping parsing and validation in the domain linker (`<domain>/link.rs`), not in `build_src`, unless it is purely KDL-syntax-related.
* Keep `Spec` structs stable and minimal; they are a contract between build-time compilers and runtime loaders.
* If you touch shared types like `MetaCommonSpec`, update build-time copies and ensure postcard decode compatibility.
* Run the feature matrix locally for registry changes:

  * `cargo test -p xeno-registry`
  * `cargo test -p xeno-registry --no-default-features`
  * `cargo test -p xeno-registry --all-features`
  * `cargo clippy -p xeno-registry --all-targets`
  * `cargo doc -p xeno-registry --no-deps` (with rustdoc link checks if relevant)
  