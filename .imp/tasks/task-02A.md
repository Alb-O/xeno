# Evildoer Registry Migration: Complete Registry-First Refactor

## Model Directive

Complete the registry-first architecture migration for Evildoer editor. Move all remaining distributed slice registries from `manifest/` and `stdlib/` to self-contained crates under `crates/registry/`. Each registry crate owns its types, macros, distributed slice, and standard implementations.

**Already migrated:**
- `menus` - Menu groups and items
- `motions` - Cursor movement primitives  
- `options` - Configuration options
- `statusline` - Statusline segments
- `text_objects` - Text object selection

**Remaining registries to migrate:**
1. `notifications` - Notification types (5 types, proc macro)
2. `commands` - Ex-mode commands (19 commands)
3. `hooks` - Event lifecycle observers (complex, proc macro)
4. `actions` - Editor actions (87 actions, result dispatch)
5. `panels` - Panel definitions (2 slices)

---

## Implementation Expectations

<mandatory_execution_requirements>

This is not a review task. When given implementation requests:

1. Edit files using tools to modify actual source files
2. Debug and fix by running builds, reading errors, iterating until it compiles
3. Run `cargo check --workspace` after each registry migration
4. Run `cargo test --workspace` after completing all migrations
5. Complete the full implementation; do not stop at partial solutions

Unacceptable responses:

- "Here's how you could implement this..."
- Providing code blocks without writing them to files
- Stopping after encountering the first error
- Leaving any registry partially migrated

</mandatory_execution_requirements>

---

## Behavioral Constraints

<verbosity_and_scope_constraints>

- Match existing registry crate patterns exactly (see `crates/registry/motions/`, `crates/registry/statusline/`)
- No inline comments narrating obvious control flow
- No decorative section markers or separators
- Keep docstrings technical and rustdoc-compatible
- Update callsites directly - no re-export wrapper layers in manifest
- Remove old code from manifest/stdlib after migration - no dead code

</verbosity_and_scope_constraints>

<design_freedom>

- Proc macros (`register_notification!`, `define_events!`) remain in `evildoer-macro` but update their references to point to new registry crate paths
- Runtime constructs (Notification builder, Editor impl) stay in their current locations
- RegistryMetadata impls remain in manifest (bridge between registry types and manifest trait)

</design_freedom>

---

## Implementation Roadmap

### Phase 1: Notifications Registry

Objective: Migrate notification type definitions and registrations.

**1.1 Create `crates/registry/notifications/`**

Files to create:
- `Cargo.toml` - Dependencies: `evildoer-registry-motions`, `linkme`, `thiserror`
- `src/lib.rs` - Types: `Level`, `Anchor`, `Animation`, `AutoDismiss`, `Timing`, `NotificationError`, `AnimationPhase`, `Overflow`, `SizeConstraint`, `SlideDirection`, `NotificationTypeDef`, `NOTIFICATION_TYPES` slice, `find_notification_type()`
- `src/impls/mod.rs` - Module declarations
- `src/impls/defaults.rs` - 5 default registrations (INFO, WARN, ERROR, SUCCESS, DEBUG)

Re-export `RegistrySource` from motions. Do NOT duplicate enum definitions.

**1.2 Update `crates/macro/src/notification.rs`**

Change all `evildoer_manifest::notifications::` paths to `evildoer_registry::notifications::`.

**1.3 Wire up workspace**

- Add `crates/registry/notifications` to root `Cargo.toml` members
- Add `evildoer-registry-notifications` to workspace.dependencies
- Add dependency to `crates/registry/Cargo.toml`
- Add re-exports to `crates/registry/src/lib.rs`

**1.4 Update manifest**

- Replace `crates/manifest/src/notifications.rs` with RegistryMetadata impl only
- Update `crates/manifest/src/lib.rs` to re-export from registry

**1.5 Update stdlib**

- Update `crates/stdlib/src/notifications/` to import from registry
- Remove `defaults.rs` (moved to registry)
- Keep `notification/` (runtime builder) and `types.rs` (re-exports)

Done: `cargo check --workspace` passes

---

### Phase 2: Commands Registry

Objective: Migrate command definitions and implementations.

**2.1 Create `crates/registry/commands/`**

Files to create:
- `Cargo.toml` - Dependencies: `evildoer-registry-motions`, `linkme`, `paste`
- `src/lib.rs` - Types: `CommandDef`, `CommandHandler`, `COMMANDS` slice, `flags` module, lookup functions
- `src/macros.rs` - `command!` macro (move from `manifest/src/macros/registry.rs`)
- `src/impls/mod.rs` - Module declarations
- `src/impls/*.rs` - All 19 command implementations from `stdlib/src/commands/`

**2.2 Wire up workspace**

Same pattern as notifications.

**2.3 Update manifest**

- Replace definitions in `crates/manifest/src/commands.rs` with RegistryMetadata impl
- Remove `command!` macro from `crates/manifest/src/macros/registry.rs`
- Update lib.rs re-exports

**2.4 Update stdlib**

- Remove `crates/stdlib/src/commands/` directory entirely
- Update lib.rs

Done: `cargo check --workspace` passes

---

### Phase 3: Panels Registry

Objective: Migrate panel definitions.

**3.1 Create `crates/registry/panels/`**

Files to create:
- `Cargo.toml`
- `src/lib.rs` - Types: `PanelDef`, `PanelId`, `PanelFactory`, `PanelFactoryDef`, `PANELS` slice, `PANEL_FACTORIES` slice, lookup functions
- `src/macros.rs` - `panel!` macro (move from `manifest/src/macros/panels.rs`)

Note: Panel implementations likely stay in `api/` since they have UI dependencies. Only move type definitions and registration infrastructure.

**3.2 Wire up workspace**

Same pattern.

**3.3 Update manifest**

- Replace `crates/manifest/src/panels.rs` with RegistryMetadata impl
- Remove `panel!` macro from manifest
- Update lib.rs re-exports

Done: `cargo check --workspace` passes

---

### Phase 4: Hooks Registry

Objective: Migrate hook event definitions and registrations. Most complex due to proc macro code generation.

**4.1 Create `crates/registry/hooks/`**

Files to create:
- `Cargo.toml` - Dependencies: `evildoer-registry-motions`, `linkme`, `paste`, `tracing`
- `src/lib.rs` - Types: `HookDef`, `HookEvent`, `HookEventData`, `OwnedHookContext`, `HookContext`, `MutableHookContext`, `HookHandler`, `HookMutability`, `HookResult`, `HookAction`, `HookScheduler`, `BoxFuture`, `HOOKS` slice, emit functions
- `src/macros.rs` - `hook!`, `async_hook!` macros (move from `manifest/src/macros/hooks.rs`)
- `src/impls/mod.rs` - Module declarations  
- `src/impls/*.rs` - Hook implementations from `stdlib/src/hooks/`

**4.2 Update `crates/macro/src/events.rs`**

Change `evildoer_manifest::hooks::` paths to `evildoer_registry::hooks::`.

The `define_events!` proc macro generates `HookEvent`, `HookEventData`, `OwnedHookContext`, and extractor macros. These generated types must reference the registry crate.

**4.3 Wire up workspace**

Same pattern.

**4.4 Update manifest**

- The `define_events!` macro invocation stays in manifest (`crates/manifest/src/hooks.rs`) but generated code references registry types
- Replace type definitions with RegistryMetadata impl
- Update lib.rs re-exports

**4.5 Update stdlib**

- Remove `crates/stdlib/src/hooks/` directory
- Update lib.rs

Done: `cargo check --workspace` passes

---

### Phase 5: Actions Registry

Objective: Migrate action definitions. Most complex - 87 actions with result dispatch system.

**5.1 Create `crates/registry/actions/`**

Files to create:
- `Cargo.toml` - Dependencies: `evildoer-registry-motions`, `evildoer-macro`, `linkme`, `paste`
- `src/lib.rs` - Types: `ActionDef`, `ActionHandler`, `ActionContext`, `ActionArgs`, `ActionMode`, `ActionResult` (with `#[derive(DispatchResult)]`), `PendingAction`, `PendingKind`, `EditAction`, `ObjectSelectionKind`, `ScrollAmount`, `ScrollDir`, `VisualDirection`, `ACTIONS` slice, `KEYBINDINGS` slice, result handler slices, dispatch infrastructure
- `src/macros.rs` - `action!` macro (move from `manifest/src/macros/actions.rs`)
- `src/impls/mod.rs` - Module declarations
- `src/impls/*.rs` - All action implementations from `stdlib/src/actions/`
- `src/result_handlers/` - Result handler implementations from `stdlib/src/editor_ctx/result_handlers/`

**5.2 Wire up workspace**

Same pattern.

**5.3 Update manifest**

- Replace `crates/manifest/src/actions/` with RegistryMetadata impl
- Remove `action!` macro
- Update lib.rs re-exports

**5.4 Update stdlib**

- Remove `crates/stdlib/src/actions/` directory
- Keep `crates/stdlib/src/editor_ctx/` (EditorCapabilities trait impls)
- Update lib.rs

Done: `cargo check --workspace` passes

---

### Phase 6: Final Cleanup

Objective: Remove dead code, verify build, run tests.

**6.1 Cleanup manifest**

- Remove empty macro files
- Remove unused imports
- Verify only RegistryMetadata impls and re-exports remain for migrated registries

**6.2 Cleanup stdlib**

- Verify only runtime constructs remain (Notification builder, EditorCapabilities impls)
- Remove any orphaned modules

**6.3 Full verification**

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace`

Done: All checks pass, no warnings

---

## Architecture

### Registry Crate Pattern

Each registry crate follows this structure:

```
crates/registry/{name}/
├── Cargo.toml
└── src/
    ├── lib.rs       # Types, distributed slice, RegistrySource re-export, lookup fns
    ├── macros.rs    # Registration macro(s)
    └── impls/       # Standard implementations
        ├── mod.rs
        └── *.rs
```

### Dependency Graph

```
evildoer-registry-motions  (base: RegistrySource, Capability, flags, movement)
    ↑
evildoer-registry-{menus,options,text_objects,statusline,notifications,commands,panels,hooks,actions}
    ↑
evildoer-registry  (umbrella: re-exports all)
    ↑
evildoer-manifest  (RegistryMetadata impls, remaining definitions)
    ↑
evildoer-stdlib    (runtime impls: Notification builder, EditorCapabilities)
```

### RegistryMetadata Bridge Pattern

For each migrated type, manifest contains only:

```rust
// crates/manifest/src/{registry_name}.rs
use evildoer_registry::{registry_name}::TypeDef;

impl crate::RegistryMetadata for TypeDef {
    fn id(&self) -> &'static str { self.id }
    fn name(&self) -> &'static str { self.name }
    fn priority(&self) -> i16 { self.priority }
    fn source(&self) -> crate::RegistrySource {
        match self.source {
            evildoer_registry::RegistrySource::Builtin => crate::RegistrySource::Builtin,
            evildoer_registry::RegistrySource::Crate(name) => crate::RegistrySource::Crate(name),
            evildoer_registry::RegistrySource::Runtime => crate::RegistrySource::Runtime,
        }
    }
}
```

### Proc Macro Updates

Proc macros in `evildoer-macro` that generate registry registrations must be updated to reference `evildoer_registry::` paths:

- `register_notification!` → `evildoer_registry::notifications::`
- `define_events!` → `evildoer_registry::hooks::`

Declarative macros move entirely to their registry crates.

---

## Anti-Patterns

1. **Re-export wrapper layers**: Don't create intermediate re-export modules. Update callsites directly to import from registry.

2. **Duplicate type definitions**: Each type exists in exactly one place. Re-export `RegistrySource` and `Capability` from motions registry.

3. **Partial migrations**: Each registry must be fully migrated before moving to the next. No half-complete states.

4. **Dead code**: Remove old definitions immediately after migration. Don't leave commented-out code.

5. **Verbose comments**: No inline comments explaining obvious code. Docstrings for public API only.

---

## Success Criteria

1. All 5 remaining registries migrated to `crates/registry/`
2. `cargo check --workspace` passes
3. `cargo test --workspace` passes  
4. `cargo clippy --workspace` has no warnings
5. No duplicate type definitions across crates
6. manifest contains only RegistryMetadata impls for migrated types
7. stdlib contains only runtime constructs (Notification builder, EditorCapabilities)
8. All imports updated to use registry paths directly

---

## Reference Files

Study these completed migrations for patterns:

- `crates/registry/motions/src/lib.rs` - Base registry pattern
- `crates/registry/statusline/src/lib.rs` - Simple registry pattern
- `crates/registry/text_objects/src/lib.rs` - Registry with multiple macros
- `crates/manifest/src/motions.rs` - RegistryMetadata impl pattern
- `crates/manifest/src/statusline.rs` - RegistryMetadata impl pattern

Files to migrate:

- `crates/manifest/src/notifications.rs` - Notification types
- `crates/manifest/src/commands.rs` - Command types
- `crates/manifest/src/panels.rs` - Panel types
- `crates/manifest/src/hooks.rs` - Hook types (uses define_events! proc macro)
- `crates/manifest/src/actions/` - Action types and result dispatch
- `crates/stdlib/src/commands/` - Command implementations
- `crates/stdlib/src/hooks/` - Hook implementations
- `crates/stdlib/src/actions/` - Action implementations
- `crates/stdlib/src/notifications/defaults.rs` - Notification registrations
- `crates/macro/src/notification.rs` - Proc macro to update
- `crates/macro/src/events.rs` - Proc macro to update
