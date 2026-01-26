# Registry System Architecture

Xeno uses a decentralized registry pattern to manage extensible editor components (actions,
commands, motions, text objects, options, themes, gutters, statusline, hooks, notifications).
Most builtins live in `crates/registry` and register themselves at compile time using
`inventory`, while lookups are centralized for O(1) resolution and strict invariants.

## Core Concepts

### Registry Metadata
The foundation of the registry is `RegistryMeta`, defined in `xeno-registry-core`.
Every registered item provides:

- ID: A unique, sacred identifier (typically `crate::name`).
- Name: The primary human-readable lookup key.
- Aliases: Alternative strings for lookup.
- Priority: Used for collision resolution (higher wins).
- Source: Indicates if the item is Builtin, from a Crate, or added at Runtime.
- Required Capabilities: Slice of editor capabilities needed for execution.

### IDs and Stability
Registry macros generate IDs with `concat!(env!("CARGO_PKG_NAME"), "::", name)`. Since builtins
are consolidated in the `xeno-registry` crate, their IDs look like `xeno-registry::left`.
Use ID constants from `xeno-primitives` (for example `xeno_primitives::motion_ids::LEFT`) when
requesting motions instead of hardcoding strings.

### The Trait Trio
Registration and introspection are handled by three related traits:
1. `RegistryMeta`: The raw data structure.
2. `RegistryEntry`: Requires `fn meta(&self) -> &RegistryMeta`.
3. `RegistryMetadata`: Simplified type-erased access to core fields.

The macro `impl_registry_entry!` boilerplates these traits for any struct with a
`meta: RegistryMeta` field.

## Registration Flow

### Compile-time (Inventory)
Most components register using the `inventory` crate via `xeno-registry` wrappers:
1. `xeno_registry::inventory::Reg<T>` registers a single definition.
2. `xeno_registry::inventory::RegSlice<T>` registers a slice (used for keybindings, prefixes).
3. Macros like `action!`, `command!`, `motion!`, `text_object!`:
   - Generate a `static` definition.
   - Create a typed `Key<T>` handle.
   - Submit `Reg(&static_def)` to inventory.

Example action that emits a motion by ID:

```rust
action!(move_left, {
    description: "Move cursor left",
    bindings: r#"normal "h" "left"
insert "left""#,
}, |ctx| cursor_motion(ctx, motion_ids::LEFT));
```

This expands into:
- An `ActionDef` registration.
- A `KeyBinding` registration (when bindings are provided).
- A typed `ActionKey` constant.

### RegistryDb Construction (`db` feature)
When `xeno-registry` is built with the `db` feature, startup builds a `RegistryDb`:
1. `RegistryDbBuilder` collects definitions from inventory (`Reg<T>` or domain-specific regs).
2. Plugin entries registered with `register_plugin!` can extend the builder.
3. Each domain is built into a `RegistryIndex` via `RegistryBuilder::build`.
4. Indexes are wrapped in `RuntimeRegistry` for runtime overlays.

### RegistryBuilder Two-pass Construction
`RegistryBuilder` (in `xeno-registry-core`) enforces invariants:
- Pass 1: Insert all IDs (duplicate IDs are fatal).
- Pass 2: Insert names/aliases (ID shadowing is fatal; collisions resolved by `DuplicatePolicy`).

## Lookup Mechanics

### ID-first Lookup
`RegistryIndex::get` checks the ID namespace first, then falls back to name/alias lookups.
This guarantees ID lookups resolve to the intended definition even if names collide.

### Secondary Indexes
Some registries build extra indices for efficient domain-specific lookup:
- `TEXT_OBJECT_TRIGGER_INDEX` (trigger character -> text object).
- `OPTION_KDL_INDEX` (KDL key -> option).
- `BUILTIN_HOOK_BY_EVENT` (event -> hooks, sorted by priority).

## Runtime Extensibility

`RuntimeRegistry<T>` provides a thread-safe overlay for adding definitions after startup:
- Layered lookup: ID (Extras then Builtins) then Name/Alias (Extras then Builtins).
- Atomic registration: all-or-nothing insertion under a write lock.
- Deterministic resolution: `DuplicatePolicy::ByPriority` orders by Priority, Source rank,
  then ID.

## Handles and Safety

The `Key<T>` type is a typed wrapper around `&'static T`. It enforces that a "Motion handle"
cannot be used where an "Action handle" is expected while remaining zero-cost at runtime.

## Collision Diagnostics

Registries track name/alias collisions during construction. These are accessible via
`RegistryIndex::collisions()` for debugging or warning users about shadowed definitions.
