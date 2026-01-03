# Evildoer: Typed Handle Cleanup - Shared Infrastructure & Final Migration

## Model Directive

Complete the typed handle migration by consolidating shared infrastructure and fixing the remaining string-based lookup. This is a cleanup/polish task.

______________________________________________________________________

## Part 1: Shared Registry Core

### Current State

`Key<T>` is defined in `evildoer-registry-motions` and re-exported by `evildoer-registry-panels`:

```rust
// crates/registry/motions/src/lib.rs
pub struct Key<T: 'static>(&'static T);
pub type MotionKey = Key<MotionDef>;

// crates/registry/panels/src/lib.rs
pub use evildoer_registry_motions::Key;
pub type PanelKey = Key<PanelIdDef>;
```

This is awkward - panels depends on motions just for the `Key` type.

### Solution: Create `evildoer-registry-core`

New minimal crate with shared registry infrastructure:

```
crates/registry/core/
├── Cargo.toml
└── src/
    └── lib.rs
```

**Contents:**

```rust
// crates/registry/core/src/lib.rs

/// Typed handle to a registry definition.
/// 
/// Zero-cost wrapper around a static reference. Provides compile-time
/// safety for internal registry references.
#[derive(Copy, Clone)]
pub struct Key<T: 'static>(&'static T);

impl<T> Key<T> {
    pub const fn new(def: &'static T) -> Self { Self(def) }
    pub const fn def(self) -> &'static T { self.0 }
}

impl<T: RegistryMetadata> Key<T> {
    pub fn name(self) -> &'static str { self.0.name() }
}

impl<T: RegistryMetadata> core::fmt::Debug for Key<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Key").field(&self.0.name()).finish()
    }
}

/// Where a registry item was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistrySource {
    Builtin,
    Crate(&'static str),
    Extension(&'static str),
    User,
}

/// Common metadata for registry items.
pub trait RegistryMetadata {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn priority(&self) -> i16;
    fn source(&self) -> RegistrySource;
}

/// Macro to implement RegistryMetadata for a type.
#[macro_export]
macro_rules! impl_registry_metadata {
    ($ty:ty) => {
        impl $crate::RegistryMetadata for $ty {
            fn name(&self) -> &'static str { self.name }
            fn description(&self) -> &'static str { self.description }
            fn priority(&self) -> i16 { self.priority }
            fn source(&self) -> $crate::RegistrySource { self.source }
        }
    };
}
```

### Migration Steps

1. Create `crates/registry/core/` with `Cargo.toml` and `src/lib.rs`
1. Move `Key<T>`, `RegistrySource`, `RegistryMetadata`, `impl_registry_metadata!` from motions
1. Update `evildoer-registry-motions` to depend on and re-export from core
1. Update `evildoer-registry-panels` to depend on core directly
1. Update `evildoer-registry-actions` if needed
1. Add to workspace `Cargo.toml`

______________________________________________________________________

## Part 2: ActionKey for delete_back

### Current State

```rust
// crates/input/src/insert.rs:22
let id = resolve_action_id("delete_back").expect("delete_back action not registered");
```

This is the only remaining internal string-based action lookup.

### Solution

Add `ActionKey` following the same pattern as `MotionKey` and `PanelKey`:

**Phase 1: Add action_id! macro and ActionKey**

```rust
// crates/registry/actions/src/lib.rs
pub type ActionKey = Key<ActionDef>;

// crates/registry/actions/src/macros.rs
// action! macro already generates statics, just need to also generate keys
```

**Phase 2: Generate keys from action! macro**

```rust
// Modify action! to generate:
pub mod keys {
    pub const delete_back: ActionKey = ActionKey::new(&ACTION_delete_back);
}
```

**Phase 3: Update insert.rs**

```rust
// crates/input/src/insert.rs
use evildoer_registry_actions::keys as actions;

if key.is_backspace() {
    return KeyResult::ActionById {
        id: actions::delete_back.id(),  // ActionKey needs .id() method
        count: 1,
        extend: false,
        register: None,
    };
}
```

**Note:** ActionKey needs an `.id()` method that returns `ActionId`. This requires ActionDef to store its ActionId, or compute it at lookup time.

### Alternative: Simpler Approach

If adding ActionKey is complex due to ActionId mechanics, a simpler fix:

```rust
// crates/registry/actions/src/lib.rs
pub mod keys {
    use super::*;
    use std::sync::OnceLock;
    
    static DELETE_BACK_ID: OnceLock<ActionId> = OnceLock::new();
    
    pub fn delete_back() -> ActionId {
        *DELETE_BACK_ID.get_or_init(|| {
            resolve_action_id("delete_back").expect("delete_back not registered")
        })
    }
}

// Usage:
use evildoer_registry_actions::keys as actions;
actions::delete_back()  // returns ActionId
```

This caches the lookup at first use, providing effective compile-time safety (if the action doesn't exist, it panics on first use during init).

______________________________________________________________________

## Part 3: Audit & Cleanup

### Verify no string lookups remain

```bash
grep -rn 'resolve_action_id\|find_action\|find_motion\|find_panel\|find_command' \
    crates/ --include='*.rs' | grep '"' | grep -v test
```

Should return nothing after migration.

### Ensure consistent re-exports

All registry crates should:

1. Depend on `evildoer-registry-core`
1. Re-export `Key`, `RegistrySource`, `RegistryMetadata` if needed
1. Have a `keys` module if they have typed handles

### Documentation

Update AGENTS.md if any new patterns are added.

______________________________________________________________________

## Files to Create/Modify

```
# New crate
crates/registry/core/Cargo.toml          # NEW
crates/registry/core/src/lib.rs          # NEW

# Update existing
Cargo.toml                               # Add core to workspace
crates/registry/motions/Cargo.toml       # Depend on core
crates/registry/motions/src/lib.rs       # Re-export from core, remove local defs
crates/registry/panels/Cargo.toml        # Depend on core instead of motions
crates/registry/panels/src/lib.rs        # Import from core
crates/registry/actions/Cargo.toml       # Depend on core
crates/registry/actions/src/lib.rs       # Add ActionKey or keys module
crates/input/src/insert.rs               # Use typed action reference
```

______________________________________________________________________

## Implementation Order

1. Create `evildoer-registry-core` crate
1. Move shared types from motions to core
1. Update motions to depend on and re-export from core
1. Update panels to depend on core directly
1. Add ActionKey or cached lookup for `delete_back`
1. Update `insert.rs` to use typed reference
1. Audit for any remaining string lookups
1. Final test pass

______________________________________________________________________

## Success Criteria

- [ ] `evildoer-registry-core` crate exists with `Key<T>`, `RegistrySource`, `RegistryMetadata`
- [ ] `evildoer-registry-motions` depends on core, re-exports shared types
- [ ] `evildoer-registry-panels` depends on core directly (not motions for Key)
- [ ] `delete_back` in insert.rs uses typed reference
- [ ] No internal string-based registry lookups remain (outside tests)
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean

______________________________________________________________________

## Complexity Notes

- **registry-core creation**: Straightforward, just moving code
- **ActionKey**: More complex due to ActionId mechanics - consider simpler OnceLock approach
- **Breaking changes**: None expected, all changes are internal refactoring
