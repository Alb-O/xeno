# Evildoer Orthogonality Refactor: Agent Specification

## Model Directive

This specification guides GPT-5.2 through a systematic refactor of the evildoer text editor to achieve true architectural orthogonality. The goal is to eliminate tight coupling, add missing event hooks, and make the extension system fully composable.

**Current state**: Registry-based actions/motions/commands work well. Panels, result handlers, and UI are tightly coupled.

**Target state**: All subsystems communicate through events and registries. Extensions can intercept any behavior.

---

## CRITICAL: Implementation Expectations

<mandatory_execution_requirements>

This is NOT a documentation-only task. When given implementation requests:

1. EDIT FILES using tools to modify actual source files
2. DEBUG AND FIX by running `cargo build`, reading errors, iterating until it compiles
3. TEST CHANGES with `cargo test --workspace`
4. COMPLETE FULL IMPLEMENTATION; do not stop at partial solutions
5. COMMIT after each phase with descriptive message

Unacceptable responses:
- "Here's how you could implement this..."
- Providing code blocks without writing them to files
- Stopping after encountering the first error
- Skipping verification steps

</mandatory_execution_requirements>

---

## Behavioral Constraints

<verbosity_and_scope_constraints>

- Produce MINIMAL code changes that satisfy the requirement
- PREFER editing existing files over creating new ones
- NO extra features, no added components, no architectural embellishments
- Follow existing macro patterns exactly (`define_events!`, `action!`, `hook!`)
- If any instruction is ambiguous, choose the simplest valid interpretation
- Each event/trait addition should be < 50 lines unless complexity demands more

</verbosity_and_scope_constraints>

<design_system_enforcement>

- Use `define_events!` proc macro for ALL new events (single source of truth)
- Use `linkme::distributed_slice` for ALL registries
- Use capability traits for ALL editor access patterns
- Do NOT invent new patterns when existing ones suffice
- Match existing code style: snake_case, doc comments on public items

</design_system_enforcement>

---

## Implementation Roadmap

### Phase 1: Focus & Layout Events

**Objective**: Make focus and layout changes observable via the event system.

**Files to modify**:
- `crates/manifest/src/hooks.rs` - Add event definitions
- `crates/api/src/editor/focus.rs` - Emit focus events
- `crates/api/src/editor/splits.rs` - Emit layout events

**Tasks**:

| ID | Task | Steps | Done when |
|----|------|-------|-----------|
| 1.1 | Add `ViewFocusChanged` event | Edit `hooks.rs`: add to `define_events!` with `view_id: ViewId, prev_view_id: OptionViewId` | Event compiles |
| 1.2 | Add `SplitCreated` event | Edit `hooks.rs`: add with `view_id: ViewId, direction: SplitDirection` | Event compiles |
| 1.3 | Add `SplitClosed` event | Edit `hooks.rs`: add with `view_id: ViewId` | Event compiles |
| 1.4 | Add `PanelToggled` event | Edit `hooks.rs`: add with `panel_id: Str, visible: Bool` | Event compiles |
| 1.5 | Emit `ViewFocusChanged` in `focus_view_inner()` | Edit `focus.rs` L31-48: call `emit_hook_event!` after focus change | Event fires on `:bn` |
| 1.6 | Emit `SplitCreated` in `split_horizontal/vertical()` | Edit `splits.rs` L21-38: emit after split creation | Event fires on `<C-w>s` |
| 1.7 | Emit `SplitClosed` in `close_view()` | Edit `splits.rs` L95-143: emit before removal | Event fires on `<C-w>q` |
| 1.8 | Emit `PanelToggled` in `toggle_panel()` | Edit `splits.rs` L63-77: emit after toggle | Event fires on panel toggle |

**Verification**: `cargo build && cargo test --workspace`

---

### Phase 2: Cursor & Selection Events

**Objective**: Emit events when cursor/selection changes via result handlers.

**Files to modify**:
- `crates/manifest/src/hooks.rs` - Verify events exist (they do: `CursorMove`, `SelectionChange`)
- `crates/stdlib/src/editor_ctx/result_handlers/core.rs` - Emit events

**Tasks**:

| ID | Task | Steps | Done when |
|----|------|-------|-----------|
| 2.1 | Emit `CursorMove` in `Motion` handler | Edit `core.rs` L29-35: emit after setting cursor | Event fires on `h/j/k/l` |
| 2.2 | Emit `SelectionChange` in selection handlers | Edit `core.rs`: emit in `InsertWithMotion`, `Selection` handlers | Event fires on `v` motions |

**Verification**: `cargo build && cargo test --workspace`

---

### Phase 3: Action Lifecycle Events

**Objective**: Make action execution observable for logging, undo grouping, command palettes.

**Files to modify**:
- `crates/manifest/src/hooks.rs` - Add events
- `crates/api/src/editor/actions_exec.rs` - Emit events

**Tasks**:

| ID | Task | Steps | Done when |
|----|------|-------|-----------|
| 3.1 | Add `ActionPre` event | Edit `hooks.rs`: add with `action_id: Str` | Compiles |
| 3.2 | Add `ActionPost` event | Edit `hooks.rs`: add with `action_id: Str, result_variant: Str` | Compiles |
| 3.3 | Emit `ActionPre` before action execution | Edit `actions_exec.rs` L9-20: emit before `action.handler(ctx)` | Fires before action |
| 3.4 | Emit `ActionPost` after result dispatch | Edit `actions_exec.rs` L70-86: emit after dispatch completes | Fires after action |

**Verification**: `cargo build && cargo test --workspace`

---

### Phase 4: Panel System Extensibility

**Objective**: Allow extensions to register custom panel types at compile-time.

**Files to modify**:
- `crates/manifest/src/panels.rs` - Add `PanelBehavior` trait
- `crates/api/src/editor/views.rs` - Replace hardcoded checks
- `crates/api/src/editor/input/key_handling.rs` - Use trait-based dispatch

**Tasks**:

| ID | Task | Steps | Done when |
|----|------|-------|-----------|
| 4.1 | Add `PanelBehavior` trait to `PanelDef` | Edit `panels.rs`: add `captures_input: bool`, `supports_window_mode: bool` fields | Compiles |
| 4.2 | Update terminal panel registration | Edit `api/src/panels/mod.rs`: set `captures_input: true` for terminal | Compiles |
| 4.3 | Replace `is_terminal_focused()` with trait check | Edit `views.rs` L44-48: check `panel.captures_input` instead of `id == "terminal"` | No hardcoded strings |
| 4.4 | Replace `is_debug_focused()` with trait check | Edit `views.rs` L51-55: check panel properties | No hardcoded strings |
| 4.5 | Update `handle_key_event()` to use panel traits | Edit `key_handling.rs` L87-98: dispatch based on `PanelDef` fields | Terminal input works |
| 4.6 | Update `actions_exec.rs` terminal checks | Edit `actions_exec.rs` L45, L125: use panel trait, not string match | No "terminal" strings |

**Verification**: `cargo build && cargo test --workspace` + manual test terminal panel

---

### Phase 5: Granular Buffer Operations

**Objective**: Split monolithic `BufferOpsAccess` into composable traits.

**Files to modify**:
- `crates/manifest/src/editor_ctx/capabilities.rs` - Split trait
- `crates/api/src/capabilities.rs` - Split implementations
- `crates/stdlib/src/actions/window.rs` - Update imports

**Tasks**:

| ID | Task | Steps | Done when |
|----|------|-------|-----------|
| 5.1 | Extract `SplitOps` trait | Edit `capabilities.rs`: move `split_horizontal`, `split_vertical`, `close_view` to new trait | Compiles |
| 5.2 | Extract `PanelOps` trait | Edit `capabilities.rs`: move `toggle_panel`, `open_panel`, `close_panel` to new trait | Compiles |
| 5.3 | Extract `FocusOps` trait | Edit `capabilities.rs`: move `focus_next`, `focus_prev`, `focus_view` to new trait | Compiles |
| 5.4 | Keep `BufferOpsAccess` as superttrait | Edit `capabilities.rs`: `BufferOpsAccess: SplitOps + PanelOps + FocusOps` | Backward compatible |
| 5.5 | Update implementations | Edit `api/src/capabilities.rs`: impl each new trait for `Editor` | All tests pass |
| 5.6 | Update result handlers | Edit `stdlib/src/actions/window.rs`: use specific traits where possible | Compiles |

**Verification**: `cargo build && cargo test --workspace`

---

### Phase 6: Result Handler Extensibility

**Objective**: Allow extensions to add handlers for existing result types.

**Files to modify**:
- `crates/manifest/src/actions/result.rs` - Add extension point
- `crates/macro/src/dispatch.rs` - Generate extension slice

**Tasks**:

| ID | Task | Steps | Done when |
|----|------|-------|-----------|
| 6.1 | Add `RESULT_EXTENSION_HANDLERS` slice | Edit `result.rs`: add distributed slice for extension handlers | Compiles |
| 6.2 | Update `dispatch_result()` generation | Edit `dispatch.rs` L162-200: iterate extension handlers after core handlers | Compiles |
| 6.3 | Add `result_extension_handler!` macro | Edit `macros/registry.rs`: macro to register extension handlers | Compiles |
| 6.4 | Document extension handler pattern | Add doc comments explaining priority and composition | Documented |

**Verification**: `cargo build && cargo test --workspace`

---

### Phase 7: Wire Pending Capabilities

**Objective**: Complete `JumpAccess` and `MacroAccess` capability traits.

**Files to modify**:
- `crates/manifest/src/editor_ctx/capabilities.rs` - Define methods
- `crates/api/src/capabilities.rs` - Implement
- `crates/api/src/editor/mod.rs` - Add backing storage

**Tasks**:

| ID | Task | Steps | Done when |
|----|------|-------|-----------|
| 7.1 | Define `JumpAccess` methods | Edit `capabilities.rs` L135-150: add `push_jump`, `pop_jump`, `jump_older`, `jump_newer` | Compiles |
| 7.2 | Add jump list storage to `Editor` | Edit `editor/mod.rs`: add `jump_list: Vec<JumpLocation>` field | Compiles |
| 7.3 | Implement `JumpAccess` for `Editor` | Edit `api/src/capabilities.rs`: implement all methods | Tests pass |
| 7.4 | Define `MacroAccess` methods | Edit `capabilities.rs` L151-166: add `start_recording`, `stop_recording`, `play_macro` | Compiles |
| 7.5 | Add macro storage to `Editor` | Edit `editor/mod.rs`: add `macros: HashMap<char, Vec<KeyEvent>>` | Compiles |
| 7.6 | Implement `MacroAccess` for `Editor` | Edit `api/src/capabilities.rs`: implement all methods | Tests pass |
| 7.7 | Remove "not yet wired" comments | Edit `capabilities.rs`, `mod.rs`: remove outdated comments | Clean |

**Verification**: `cargo build && cargo test --workspace`

---

### Phase 8: Documentation Update

**Objective**: Update AGENTS.md with new architecture.

**Files to modify**:
- `AGENTS.md`

**Tasks**:

| ID | Task | Steps | Done when |
|----|------|-------|-----------|
| 8.1 | Document new events | Add Focus/Layout/Action events to hooks table | Updated |
| 8.2 | Document panel extensibility | Add PanelBehavior trait documentation | Updated |
| 8.3 | Document granular capabilities | Update capability table with new traits | Updated |
| 8.4 | Document result handler extension | Add section on extending result handlers | Updated |

**Verification**: Review for accuracy

---

## Architecture Reference

### Event System (define_events!)

```rust
// In crates/manifest/src/hooks.rs
define_events! {
    ViewFocusChanged => "view:focus_changed" {
        view_id: ViewId,
        prev_view_id: OptionViewId,
    },
    SplitCreated => "split:created" {
        view_id: ViewId,
        direction: SplitDirection,
    },
    // ... etc
}
```

Field type tokens:
- `ViewId` → `ViewId` (copy type)
- `Str` → `&str` / `String`
- `Bool` → `bool`
- `OptionViewId` → `Option<ViewId>`

### Capability Traits

```rust
// Granular traits
pub trait SplitOps {
    fn split_horizontal(&mut self) -> Option<ViewId>;
    fn split_vertical(&mut self) -> Option<ViewId>;
    fn close_view(&mut self, view_id: ViewId);
}

pub trait PanelOps {
    fn toggle_panel(&mut self, name: &str);
    fn open_panel(&mut self, name: &str);
    fn close_panel(&mut self, name: &str);
}

// Supertrait for backward compatibility
pub trait BufferOpsAccess: SplitOps + PanelOps + FocusOps { }
```

### Panel Behavior

```rust
pub struct PanelDef {
    pub id: &'static str,
    pub name: &'static str,
    // NEW: behavioral flags
    pub captures_input: bool,        // Panel handles its own input
    pub supports_window_mode: bool,  // Panel can enter window mode
    // ... existing fields
}
```

---

## Anti-Patterns

1. **String-based type dispatch**: Check `id == "terminal"` → Use `panel.captures_input` trait field instead
2. **Hardcoded panel lists**: Match on known panel names → Iterate registry with trait checks instead
3. **Monolithic capability traits**: 15+ methods in one trait → Split into focused sub-traits
4. **Silent state mutations**: Change focus without events → Always emit corresponding event
5. **Postdictive capability checks**: Check after action runs → Check before or use type system

---

## Tool Usage Rules

<tool_usage_rules>

- Parallelize independent file reads when exploring (Read `hooks.rs`, `focus.rs`, `splits.rs` together)
- After any Edit, run `cargo build` to verify
- After completing a phase, run `cargo test --workspace`
- Use Grep to find all usages before renaming/moving code
- Commit after each phase passes tests

</tool_usage_rules>

---

## User Updates Spec

<user_updates_spec>

- Send brief updates (1-2 sentences) only when:
  - You start a new phase of work
  - You discover something that changes the plan
  - A phase completes successfully
- Avoid narrating routine tool calls ("reading file...", "running build...")
- Each update must include concrete outcome ("Added 4 events to define_events!", "Phase 1 complete, all tests pass")
- Do not expand beyond the phase you're working on

</user_updates_spec>

---

## Success Criteria

Phase complete when:
1. `cargo build` succeeds with no warnings
2. `cargo test --workspace` passes
3. No hardcoded "terminal"/"debug" strings remain (after Phase 4)
4. All new events documented in AGENTS.md (after Phase 8)

Full refactor complete when:
- All 8 phases pass verification
- Extensions can observe focus, layout, cursor, action lifecycle via events
- Extensions can register custom panels with behavioral traits
- Extensions can add handlers for existing result types
- Jump and macro capabilities are wired and functional
