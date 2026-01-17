# Registry Core - Agent Notes

Future considerations and architectural notes from code review.

## RuntimeRegistry Performance

Current implementation uses `RwLock<RuntimeExtras<T>>` which takes a read lock on every `get()` call. This is acceptable for current usage but has upgrade paths if `get()` becomes hot:

### Freeze-After-Init Pattern

If runtime registration only happens during startup/plugin-load, consider:

```rust
pub struct RuntimeRegistry<T> {
    builtins: RegistryIndex<T>,
    // Replace RwLock with atomic state machine
    state: AtomicState<T>,
}

enum AtomicState<T> {
    // During init: accepts registrations
    Mutable(RwLock<RuntimeExtras<T>>),
    // After freeze: lock-free reads from merged index
    Frozen(RegistryIndex<T>),
}

impl<T> RuntimeRegistry<T> {
    /// Call after all plugins loaded. Merges extras into builtins for lock-free reads.
    pub fn freeze(&self) { ... }
}
```

### Snapshot Swapping (ArcSwap)

If registration can happen at any time but is rare:

```rust
use arc_swap::ArcSwap;

pub struct RuntimeRegistry<T> {
    builtins: RegistryIndex<T>,
    extras: ArcSwap<RuntimeExtras<T>>,
}
```

Readers get a snapshot reference (no lock). Writers clone-and-swap (more expensive but rare).

## Plugin System Considerations

### Static Lifetime Constraint

Current `RuntimeRegistry` requires `&'static T` and `&'static str` keys. This works for:

- Compile-time builtins
- Crates linked at startup (inventory pattern)
- Leaked allocations (acceptable for long-lived plugins)

**Does NOT work for:**

- Truly dynamic plugins (dlopen, WASM, user scripts)
- Reloadable/unloadable plugins

### Future Plugin Architecture

If unloadable plugins are needed, consider layered scopes:

```rust
pub struct LayeredRegistry<T> {
    builtins: RegistryIndex<T>,
    // Each plugin gets its own layer
    layers: Vec<(PluginId, HashMap<String, Arc<T>>)>,
}

impl<T> LayeredRegistry<T> {
    /// Lookup: newest layer -> older layers -> builtins
    pub fn get(&self, key: &str) -> Option<&T> { ... }

    /// Remove entire plugin layer (no key-level archaeology)
    pub fn unload_plugin(&mut self, id: PluginId) { ... }
}
```

This model:

- Uses `Arc<T>` + `String` keys (no 'static requirement)
- Makes unload deterministic (remove whole layer)
- Handles shadowing naturally (newer layers win)

## DuplicatePolicy + Sort Order Interaction

With `sort_default()` (priority descending):

- `FirstWins` → highest priority wins (intended behavior)
- `LastWins` → lowest priority wins (usually wrong)

Current default `DuplicatePolicy::for_build()` returns `FirstWins` in release and `Panic` in debug, which matches the intended semantics.

## Trait Consolidation (Deferred)

Current traits:

- `RegistryMeta`: pure data struct
- `RegistryEntry`: trait requiring `fn meta(&self) -> &RegistryMeta`
- `RegistryMetadata`: additional trait for type-erased access

Consolidation would be:

```rust
pub trait HasRegistryMeta {
    fn meta(&self) -> &RegistryMeta;
}
```

Only worth doing if:

1. Boilerplate impls are annoying weekly
2. Contributors frequently misuse the trio
3. Plugin API is blocked by awkward metadata access
