# Hooks Registry - Agent Notes

Future considerations specific to the hook system.

## Event-Indexed Pattern

Hooks use a different pattern than other registries because lookup is by `HookEvent` enum, not by string name:

```
HOOKS (RegistryIndex)           → name/id lookup, iteration
BUILTIN_BY_EVENT (HashMap)      → event dispatch
EXTRA_BY_EVENT (RwLock<HashMap>) → runtime hook dispatch
```

This is correct separation: `RegistryIndex` handles identity/introspection, event maps handle dispatch.

## Ordering Guarantees

Hooks within each event are sorted by `(priority asc, name asc)`:
- Lower priority numbers run first
- Name provides stable tie-breaking

This matches the existing emit behavior in `emit.rs` which also sorts by priority.

## Alternative: Index-Based References

Current implementation stores `Vec<&'static HookDef>` in event maps. Alternative is storing indices:

```rust
pub struct HookIndex {
    pub hooks: RegistryIndex<HookDef>,
    pub by_event: HashMap<HookEvent, Vec<usize>>,
}
```

Benefits:
- Smaller memory footprint (usize vs pointer)
- No lifetime gymnastics
- Single source of truth

Tradeoffs:
- Indirect lookup (index → slice → def)
- More complex runtime registration

Current approach is fine given hook count (~10 total). Reconsider if:
- Hook count grows significantly (100+)
- Memory pressure becomes concern
- Need to support hook removal

## Runtime Registration and Event Index

Current `register_hook()` maintains sorted order via `binary_search_by()` + `insert()`.

If plugin registration becomes batch-oriented:

```rust
/// Register multiple hooks, then rebuild event index once.
pub fn register_hooks_batch(defs: &[&'static HookDef]) {
    // Add all to extras
    // Rebuild by_event from scratch
    // More efficient than N sorted inserts
}
```

## HookEvent Optimization

If `HookEvent` is a small enum (currently ~20 variants), consider:

```rust
// Instead of HashMap<HookEvent, Vec<...>>
struct EventIndex {
    // Direct indexing, no hash lookup
    by_event: [Vec<&'static HookDef>; HookEvent::COUNT],
}
```

Requires `HookEvent` to impl a trait giving `as_usize()` and `COUNT`.
