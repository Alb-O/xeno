# Registry System Architecture

Xeno uses a decentralized registry pattern to manage extensible editor components (actions, commands, motions, text objects, etc.). This system allows crates to register functionality at compile time without centralized dispatch logic, while maintaining strict invariants and providing O(1) lookup performance.

## Core Concepts

### Registry Metadata
The foundation of the registry is `RegistryMeta`, a unified metadata structure defined in `xeno-registry-core`. Every registered item must provide this metadata:

- ID: A unique, sacred identifier (typically `crate::name`).
- Name: The primary human-readable lookup key.
- Aliases: Alternative strings for lookup.
- Priority: Used for collision resolution (higher wins).
- Source: Indicates if the item is Builtin, from a Crate, or added at Runtime.
- Required Capabilities: Bitset of editor capabilities needed for execution.

### The Trait Trio
Registration and introspection are handled by three related traits:
1. `RegistryMeta`: The raw data structure.
2. `RegistryEntry`: A trait requiring `fn meta(&self) -> &RegistryMeta`.
3. `RegistryMetadata`: A simplified trait for type-erased access to core fields.

The macro `impl_registry_entry!` is provided to boilerplate-implement these traits for any struct with a `meta: RegistryMeta` field.

## Registration Flow

### Compile-time (Inventory)
Most components register themselves using the `inventory` crate.
1. A registry-specific wrapper type (e.g., `ActionReg(&'static ActionDef)`) is defined.
2. The wrapper implements `RegistryReg<T>`, exposing the inner definition.
3. Macros like `action!`, `command!`, or `motion!`:
   - Generate a `static` definition.
   - Use `inventory::submit!` to register the definition via its wrapper.

### Index Construction
Registries are typically initialized via `LazyLock` using the `RegistryBuilder`:

```rust
pub static ACTIONS: LazyLock<RuntimeRegistry<ActionDef>> = LazyLock::new(|| {
    let builtins = RegistryBuilder::new("actions")
        .extend_inventory::<ActionReg>()
        .sort_default() // Priority descending
        .build();
    RuntimeRegistry::new("actions", builtins)
});
```

`RegistryBuilder` performs a two-pass construction:
- Pass 1 (IDs): Inserts all IDs into the ID namespace. Duplicate IDs are fatal and trigger a panic.
- Pass 2 (Names/Aliases): Inserts lookup keys. If a name shadows an existing ID, it's a fatal error. If names collide, the `DuplicatePolicy` (FirstWins, LastWins, or Panic) determines the winner.

## Lookup Mechanics

### ID-First Lookup
The `RegistryIndex::get` method enforces ID-first lookup:
1. Check the ID namespace for an exact match.
2. Fall back to the Name/Alias namespace.

This ensures that an ID lookup always resolves to the intended definition, even if another definition has a name that happens to match the ID string.

### Secondary Indexes
Some registries require non-string lookups (e.g., dispatching by event enum for hooks, or by trigger character for LSP). These use a two-tiered approach:
1. `RegistryIndex` handles identity, O(1) string lookup, and introspection.
2. A secondary `HashMap` or array, built using `build_map` or custom logic, provides efficient dispatch for the primary use case.

### Co-located Configuration
The registry macros often co-locate behavior with configuration. For example, the `action!` macro accepts a KDL string for keybindings:

```rust
action!(move_left, {
    description: "Move cursor left",
    bindings: r#"normal "h" "left"
insert "left""#,
}, |ctx| cursor_motion(ctx, motions::left));
```

This macro expands into:
- An `ActionDef` registration in `ACTIONS`.
- A `KeyBindingSetReg` registration containing the parsed bindings.
- A typed `ActionKey` constant for use in other code.

### Invariants
The registry enforces three "sacred" invariants:
1. Unique IDs: Two definitions cannot share the same `meta.id`. This is fatal and triggers a panic.
2. No ID Shadowing: A name or alias cannot equal an existing ID. This is fatal.
3. Static Lifetimes: Registry items must have `'static` lifetimes, facilitating zero-cost handles.

## Runtime Extensibility


The `RuntimeRegistry<T>` type provides a thread-safe overlay for adding definitions after the editor has started (e.g., from plugins or dynamic configuration).

- Layered Lookup: Checks Runtime Extras (ID then Name) before falling back to Builtins.
- Atomic Registration: Either the definition and all its keys are successfully added, or none are (on conflict). Implementation uses a scratch clone and atomic swap under a write lock.
- Thread Safety: Uses `RwLock` for concurrent reads. Poisoning is handled by failing fast in production and allowing recovery in tests.
- Deterministic Resolution: Uses `DuplicatePolicy::ByPriority` by default, which follows a total order:
  1. Priority (Higher wins)
  2. Source Rank (Runtime > Crate > Builtin)
  3. ID (Lexical higher wins)

## Handles and Safety

The `Key<T>` type is a zero-cost, typed wrapper around a `&'static T`. It provides compile-time safety when passing registry references between systems, ensuring that a "Motion handle" cannot be accidentally used where an "Action handle" is expected.

## Collision Diagnostics

The registry tracks all name/alias collisions that occurred during construction. These can be inspected via `index.collisions()` for debugging purposes or to warn users when their configuration accidentally shadows built-in functionality.
