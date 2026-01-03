# Evildoer: Complete Typed Handle Migration - Panels, Commands, and Beyond

## Model Directive

Complete the typed handle migration across all registries where it provides value. Address the cross-crate architecture issues and fully eliminate stringly-typed internal references.

**This is the final cleanup task** - be thorough and handle all edge cases.

______________________________________________________________________

## Context

Task-04B/C/D implemented typed handles for motions (`MotionKey`). The pattern works well:

- `motion!` macro generates both slice entry and `pub const` key
- `cursor_motion(ctx, motions::left)` instead of `cursor_motion(ctx, "left")`
- Compile-time safety via Rust name resolution

Remaining work:

1. **Panels** - `ActionResult::TogglePanel("terminal")` uses strings
1. **Commands** - `ActionResult::Command { name: "...", ... }` uses strings
1. **Cross-crate architecture** - panels defined in `evildoer-api`, not registry crate

______________________________________________________________________

## Part 1: PanelKey Implementation

### The Cross-Crate Problem

Panels are defined in `evildoer-api` because they depend on API types:

```rust
// crates/api/src/panels/mod.rs
panel!(terminal, {
    factory: || Box::new(TerminalBuffer::new()),  // TerminalBuffer is in evildoer-api
});
```

But `ActionResult::TogglePanel` is in `evildoer-registry-actions`, which can't depend on `evildoer-api` (would create cycle).

### Solution Options

**Option A: Split panel identity from implementation**

```rust
// In evildoer-registry-panels (low-level)
panel_id!(terminal, { description: "Terminal emulator" });
panel_id!(debug, { description: "Debug panel" });

// In evildoer-api (high-level)  
register_panel_factory!(terminal, || Box::new(TerminalBuffer::new()));
```

Actions reference `PanelKey` from registry crate, factories registered separately.

**Option B: Accept string boundary for panels**

Panels are few and rarely change. Keep strings but add runtime validation:

```rust
// Validate at startup that all referenced panels exist
fn validate_panel_refs() {
    assert!(find_panel("terminal").is_some());
    assert!(find_panel("debug").is_some());
}
```

**Option C: Move panel toggle logic to API layer**

Instead of `ActionResult::TogglePanel(name)`, have dedicated methods:

```rust
// In evildoer-api
impl Editor {
    pub fn toggle_terminal(&mut self) { ... }
    pub fn toggle_debug(&mut self) { ... }
}

// Actions return a typed enum
ActionResult::TogglePanel(PanelToggle::Terminal)
```

### Recommended: Option A (split identity/implementation)

**Phase 1: Add panel_id! macro and PanelKey**

```rust
// crates/registry/panels/src/lib.rs
pub type PanelKey = Key<PanelIdDef>;

pub struct PanelIdDef {
    pub name: &'static str,
    pub description: &'static str,
}

// crates/registry/panels/src/macros.rs
macro_rules! panel_id {
    ($name:ident, { description: $desc:expr }) => {
        #[linkme::distributed_slice(PANEL_IDS)]
        static [<PANEL_ID_ $name>]: PanelIdDef = PanelIdDef { ... };
        
        pub mod keys {
            pub const $name: PanelKey = PanelKey::new(&super::[<PANEL_ID_ $name>]);
        }
    };
}
```

**Phase 2: Define panel IDs in registry crate**

```rust
// crates/registry/panels/src/builtins.rs
panel_id!(terminal, { description: "Embedded terminal emulator" });
panel_id!(debug, { description: "Debug log viewer" });
```

**Phase 3: Update ActionResult**

```rust
// crates/registry/actions/src/result.rs
pub enum ActionResult {
    TogglePanel(PanelKey),  // was &'static str
}
```

**Phase 4: Update actions**

```rust
// crates/registry/actions/src/impls/window.rs
use evildoer_registry_panels::keys as panels;

action!(toggle_terminal, { ... },
    |_ctx| ActionResult::TogglePanel(panels::terminal));
```

**Phase 5: Register factories in API crate**

```rust
// crates/api/src/panels/mod.rs
use evildoer_registry::panels::keys as panel_ids;

register_panel_factory!(panel_ids::terminal, || Box::new(TerminalBuffer::new()));
register_panel_factory!(panel_ids::debug, || Box::new(DebugPanel::new()));
```

______________________________________________________________________

## Part 2: CommandKey Assessment

### Current State

Commands are looked up by string at runtime:

- User types `:help` → parse → `find_command("help")` → execute
- `ActionResult::Command { name: "help", args }` queues for execution

### Analysis

Commands are **boundary-driven** - user input determines which command runs. Unlike motions (where actions hardcode motion names), command references are typically:

1. User CLI input (`:write`, `:quit`)
1. Config keybindings
1. Programmatic triggers (rare)

### Recommendation: Defer CommandKey

CommandKey provides little value because:

- Command names come from user input (can't be compile-time checked)
- Few if any internal hardcoded command references
- Runtime lookup is appropriate for boundary data

**If internal command refs exist**, consider:

```rust
// Only if we find ActionResult::Command { name: "hardcoded", ... }
pub type CommandKey = Key<CommandDef>;
command!(write, { ... });  // generates keys::write
ActionResult::Command { command: commands::write, args }
```

______________________________________________________________________

## Part 3: ActionKey Assessment

### Current State

Actions are already mostly typed via `ActionId`:

```rust
// crates/core/src/lib.rs
pub struct ActionId(u16);  // Index into ACTIONS slice
```

Keybindings resolve to `ActionId`, not strings. The `find_action("name")` is mainly for:

- Config parsing
- Debug/introspection
- Tests

### Recommendation: Keep Current ActionId System

`ActionId` already provides compile-time-ish safety (IDs are stable indices). Adding `ActionKey` would be redundant.

______________________________________________________________________

## Part 4: Other Registries

### Text Objects

- Looked up by trigger char (`'w'` for word), not name
- No internal string refs
- **Skip**

### Hooks

- Event-based dispatch, not name lookup
- **Skip**

### Statusline Segments

- Config-driven, boundary data
- **Skip**

### Menus

- UI-driven, boundary data
- **Skip**

### Themes

- Config-driven
- **Skip**

### Options

- Config-driven
- **Skip**

______________________________________________________________________

## Part 5: Cleanup and Consistency

### Audit for remaining string refs

```bash
# Find all string literals in action results
grep -rn 'ActionResult::' crates/registry/actions/src/impls/ | grep '"'
```

### Ensure all typed handle crates export keys module

```rust
// Pattern: each registry with typed handles should have
pub mod keys {
    pub use crate::impls::*;  // re-export all generated keys
}
```

### Documentation

Update AGENTS.md with:

- Typed handle pattern explanation
- When to use `*Key` vs strings
- How to add new registry items

______________________________________________________________________

## Implementation Order

1. **PanelKey infrastructure** - Add `Key<T>` to panels crate, `panel_id!` macro
1. **Built-in panel IDs** - Define terminal/debug in registry crate
1. **Update ActionResult::TogglePanel** - Change to `PanelKey`
1. **Update window.rs actions** - Use `panels::terminal` etc.
1. **Split panel! macro** - Separate ID registration from factory
1. **Update API panel definitions** - Use `register_panel_factory!`
1. **Audit and cleanup** - Find any remaining string refs
1. **Documentation** - Update AGENTS.md

______________________________________________________________________

## Files to Modify

```
# PanelKey infrastructure
crates/registry/panels/src/lib.rs           # Add PanelKey, PanelIdDef
crates/registry/panels/src/macros.rs        # Add panel_id! macro
crates/registry/panels/src/builtins.rs      # NEW: built-in panel IDs

# ActionResult changes  
crates/registry/actions/src/result.rs       # TogglePanel(PanelKey)
crates/registry/actions/src/impls/window.rs # Use panels::terminal

# Panel factory split
crates/api/src/panels/mod.rs                # register_panel_factory!
crates/registry/panels/src/lib.rs           # Separate PANEL_IDS from PANEL_FACTORIES

# Result handler update
crates/core/src/editor_ctx/result_handlers/ # Update toggle_panel handler
```

______________________________________________________________________

## Success Criteria

- [ ] `PanelKey` type exists with `panel_id!` macro
- [ ] Built-in panels (terminal, debug) have typed keys
- [ ] `ActionResult::TogglePanel(PanelKey)` instead of string
- [ ] Window actions use `panels::terminal`, `panels::debug`
- [ ] Panel factories registered separately from IDs
- [ ] No stringly-typed internal registry references remain
- [ ] `cargo test --workspace` passes
- [ ] AGENTS.md updated with typed handle pattern

______________________________________________________________________

## Edge Cases

1. **Dynamic panels** - If plugins can define panels at runtime, need string fallback
1. **Config references** - User config still uses strings, resolved at load time
1. **Serialization** - If ActionResult is serialized, PanelKey needs `name()` method

______________________________________________________________________

## Anti-Patterns to Avoid

1. **Don't over-engineer** - Only add typed handles where internal refs exist
1. **Don't break boundaries** - User input always uses strings, that's fine
1. **Don't create cycles** - Keep registry crates low-level
1. **Don't duplicate** - One source of truth for panel identity
