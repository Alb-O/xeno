# Xeno: Architectural Consistency Fixes

## Model Directive

Fix all architectural issues identified in the task-03A audit. This is a fix-all task - complete every item before reporting back.

______________________________________________________________________

## Implementation Expectations

\<mandatory_execution_requirements>

1. Fix each issue in order
1. Run `cargo check --workspace` after each fix
1. Run `cargo test --workspace` after all fixes
1. Do not stop at partial completion - fix ALL issues listed below
1. If a fix causes cascading changes, follow through to all affected files

Unacceptable:

- Fixing only some issues
- Leaving broken builds
- Reporting "here's how to fix" without actually fixing

\</mandatory_execution_requirements>

______________________________________________________________________

## Fixes Required

### Fix 1: xeno-base dependency boundary

**Location**: `crates/base/src/lib.rs:10`, `crates/base/Cargo.toml`

**Issue**: Base crate re-exports `xeno_tui::style` (Color, Style, Modifier) and has unused `crossterm` dependency. This pollutes the primitive crate with UI concerns.

**Fix**:

1. Remove `crossterm` dependency from `crates/base/Cargo.toml` if unused
1. Check who uses `xeno_base::{Color, Style, Modifier}`:
   - If only `xeno-language`, have language import directly from `xeno_tui`
   - If multiple crates need it, keep the re-export but document why
1. Remove the `xeno-tui` feature/dependency from base if possible
1. Update any affected imports

**Verify**: `cargo check --workspace`

______________________________________________________________________

### Fix 2: Registry access - remove core re-export hop

**Location**: `crates/input/src/handler.rs:6`, `crates/input/src/insert.rs:5`

**Issue**: Input crate uses `xeno_core::registry::BindingMode` creating unnecessary 3-hop chain.

**Fix**:

1. Add `xeno-registry` as direct dependency to `crates/input/Cargo.toml`
1. Change imports from `xeno_core::registry::BindingMode` to `xeno_registry::BindingMode`
1. Remove `pub use xeno_registry as registry` from `crates/core/src/lib.rs` if no longer needed elsewhere
1. Check other crates for similar patterns and fix

**Verify**: `cargo check --workspace`

______________________________________________________________________

### Fix 3: OptionValue unification

**Location**: `crates/config/src/options.rs:9`, `crates/registry/options/src/lib.rs:17`

**Issue**: Duplicate `OptionValue` enum definitions. Config references non-existent `xeno_core::OptionValue`.

**Fix**:

1. Remove duplicate `OptionValue` from `crates/config/src/options.rs`
1. Import from `xeno_registry::options::OptionValue` instead
1. Add `xeno-registry` dependency to config if not present
1. Update all usages in config crate
1. Remove broken `xeno_core::OptionValue` reference

**Verify**: `cargo check --workspace`

______________________________________________________________________

### Fix 4: Menus registry alignment

**Location**: `crates/registry/menus/src/lib.rs:10`, `crates/registry/menus/src/def.rs`

**Issue**: Menus registry defines its own `RegistrySource` instead of reusing from motions. Doesn't implement `RegistryMetadata`.

**Fix**:

1. Add `xeno-registry-motions` dependency to `crates/registry/menus/Cargo.toml`
1. Remove local `RegistrySource` definition
1. Import `RegistrySource`, `RegistryMetadata`, `impl_registry_metadata!` from motions
1. Implement `RegistryMetadata` for `MenuGroupDef` and `MenuItemDef` using the macro
1. Update `crates/registry/src/lib.rs` to re-export if needed

**Verify**: `cargo check --workspace`

______________________________________________________________________

### Fix 5: Remove orphan core macros

**Location**: `crates/core/src/macros/mod.rs`, `crates/core/src/macros/helpers.rs`

**Issue**: `__opt` and `__opt_slice` macro helpers remain in core but are unused after registry migration.

**Fix**:

1. Verify macros are truly unused: `rg "__opt" crates/`
1. If unused, delete `crates/core/src/macros/` directory
1. Remove `pub mod macros;` from `crates/core/src/lib.rs`
1. If used somewhere, document where and why

**Verify**: `cargo check --workspace`

______________________________________________________________________

### Fix 6: Remove unused config dependency on core

**Location**: `crates/config/Cargo.toml:12`

**Issue**: `xeno-core` listed as dependency but not used (only in comment).

**Fix**:

1. Remove `xeno-core` from `crates/config/Cargo.toml` dependencies
1. Remove any dead imports or comments referencing it
1. If actually needed after Fix 3, keep it but ensure it's used

**Verify**: `cargo check --workspace`

______________________________________________________________________

### Fix 7: Document macro crate

**Location**: `crates/macro/Cargo.toml`, `crates/macro/src/lib.rs`

**Issue**: No description or crate-level docs.

**Fix**:

1. Add `description = "Procedural macros for Xeno editor"` to Cargo.toml
1. Add crate-level doc comment to lib.rs explaining purpose:
   ```rust
   //! Procedural macros for Xeno editor.
   //!
   //! Provides derive macros and attribute macros:
   //! - `#[derive(DispatchResult)]` - generates result handler slices
   //! - `#[extension]` - extension registration
   //! - `define_events!` - hook event generation
   ```

**Verify**: `cargo doc -p xeno-macro --no-deps`

______________________________________________________________________

## Execution Order

1. Fix 5 (orphan macros) - simplest, no dependencies
1. Fix 6 (unused config dep) - simple removal
1. Fix 7 (macro docs) - simple addition
1. Fix 4 (menus alignment) - registry internal
1. Fix 3 (OptionValue) - cross-crate type unification
1. Fix 2 (registry re-export) - import path cleanup
1. Fix 1 (base deps) - most invasive, do last

______________________________________________________________________

## Final Verification

After all fixes:

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

All must pass with no errors or warnings.

______________________________________________________________________

## Success Criteria

- [ ] No duplicate type definitions
- [ ] No unused dependencies
- [ ] All registry crates follow shared pattern (RegistrySource from motions, impl_registry_metadata!)
- [ ] No orphan modules or macros
- [ ] Import paths are direct (no unnecessary re-export hops)
- [ ] All public crates have descriptions
- [ ] Build passes, tests pass, clippy clean
