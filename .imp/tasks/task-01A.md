# Xeno Menu Macro System: Agent Specification

## Model Directive

This document specifies the implementation of a macro-based menu registration system for the Xeno editor. The goal is to replace the hardcoded `create_menu()` function with distributed `menu_group!` and `menu_item!` macros that follow the established registry patterns used throughout the codebase (`action!`, `command!`, `hook!`, etc.).

______________________________________________________________________

## Implementation Expectations

\<mandatory_execution_requirements>

This is not a review task. When given implementation requests:

1. Edit files using tools to modify actual source files
1. Debug and fix by running builds, reading errors, iterating until it compiles
1. Test changes as appropriate (run `cargo build --workspace`, `cargo test --workspace`)
1. Complete the full implementation; do not stop at partial solutions

Unacceptable responses:

- "Here's how you could implement this..."
- Providing code blocks without writing them to files
- Stopping after encountering the first error or completing only 1 of several assigned tasks

\</mandatory_execution_requirements>

______________________________________________________________________

## Behavioral Constraints

\<verbosity_and_scope_constraints>

- Prefer editing existing files over creating new ones when it makes sense
- Avoid unnecessary features unrelated to the task
- If any instruction is ambiguous, choose the simplest valid interpretation
- Follow existing code patterns where they exist (study `action!`, `command!`, `motion!` macros)
- Match existing code style: tabs for indentation, similar doc comment patterns

\</verbosity_and_scope_constraints>

\<design_freedom>

- Explore existing patterns before proposing changes
- New abstractions or patterns are welcome when they improve code health
- Use judgment: balance consistency with the existing macro ecosystem

\</design_freedom>

______________________________________________________________________

## Implementation Roadmap

### Phase 1: Define Types and Distributed Slices

Objective: Create the foundational types and linkme slices in `crates/manifest/`

Tasks:

- 1.1 Create `crates/manifest/src/menu.rs`:

  - Define `MenuGroupDef` struct with fields: `id`, `name`, `label`, `priority`, `source`
  - Define `MenuItemDef` struct with fields: `id`, `name`, `group`, `label`, `command`, `shortcut`, `priority`, `source`
  - Declare `#[distributed_slice] pub static MENU_GROUPS: [MenuGroupDef]`
  - Declare `#[distributed_slice] pub static MENU_ITEMS: [MenuItemDef]`
  - Add helper functions: `all_groups()`, `items_for_group(group_name)`
  - Done: File exists, exports types and slices

- 1.2 Update `crates/manifest/src/lib.rs`:

  - Add `pub mod menu;`
  - Re-export `menu::{MenuGroupDef, MenuItemDef, MENU_GROUPS, MENU_ITEMS}`
  - Done: Exports visible from crate root

### Phase 2: Implement Declarative Macros

Objective: Create `menu_group!` and `menu_item!` macros in manifest

Tasks:

- 2.1 Create `crates/manifest/src/macros/menu.rs`:

  - Implement `menu_group!` macro following `action!` pattern
  - Implement `menu_item!` macro following `action!` pattern
  - Use `paste::paste!` for identifier generation
  - Use `linkme::distributed_slice` for registration
  - Support optional fields via `$crate::__opt!` and `$crate::__opt_slice!`
  - Done: Macros compile and expand correctly

- 2.2 Update `crates/manifest/src/macros/mod.rs`:

  - Add `mod menu;` if macros module exists
  - Or integrate into existing macro organization
  - Done: Macros exported from manifest crate

### Phase 3: Update API Menu Builder

Objective: Replace hardcoded menu with slice-based assembly

Tasks:

- 3.1 Update `crates/api/src/menu.rs`:
  - Import `MENU_GROUPS` and `MENU_ITEMS` from manifest
  - Rewrite `create_menu()` to:
    1. Collect and sort `MENU_GROUPS` by priority
    1. For each group, collect matching `MENU_ITEMS` and sort by priority
    1. Build `MenuItem::group()` with nested `MenuItem::item()` calls
    1. Return `MenuState::new(groups)`
  - Keep `MenuAction` enum and `process_menu_events()` unchanged
  - Done: Menu builds correctly from slices

### Phase 4: Create Stdlib Menu Definitions

Objective: Move existing menu items to macro-based registration in stdlib

Tasks:

- 4.1 Create `crates/stdlib/src/menus/mod.rs`:

  - Add module structure for menu definitions
  - Done: Module compiles

- 4.2 Create `crates/stdlib/src/menus/file.rs`:

  - Define `menu_group!(file, { label: "File", priority: 0 })`
  - Define items: `file_new`, `file_open`, `file_save`, `file_save_as`, `file_quit`
  - Done: File group and items registered

- 4.3 Create `crates/stdlib/src/menus/edit.rs`:

  - Define `menu_group!(edit, { label: "Edit", priority: 10 })`
  - Define items: `edit_undo`, `edit_redo`, `edit_cut`, `edit_copy`, `edit_paste`
  - Done: Edit group and items registered

- 4.4 Create `crates/stdlib/src/menus/view.rs`:

  - Define `menu_group!(view, { label: "View", priority: 20 })`
  - Define items: `view_split_horizontal`, `view_split_vertical`, `view_close_split`
  - Done: View group and items registered

- 4.5 Create `crates/stdlib/src/menus/help.rs`:

  - Define `menu_group!(help, { label: "Help", priority: 100 })`
  - Define items: `help_about`
  - Done: Help group and items registered

- 4.6 Update `crates/stdlib/src/lib.rs`:

  - Add `mod menus;`
  - Done: Menus module integrated

### Phase 5: Verification

Objective: Ensure everything compiles and the menu displays correctly

Tasks:

- 5.1 Build verification:

  - Run `cargo build --workspace`
  - Fix any compilation errors
  - Done: Workspace builds without errors

- 5.2 Test verification:

  - Run `cargo test --workspace`
  - Fix any test failures
  - Done: All tests pass

______________________________________________________________________

## Architecture

### Type Definitions

```rust
// crates/manifest/src/menu.rs

/// Definition of a top-level menu group (e.g., "File", "Edit").
pub struct MenuGroupDef {
    /// Full qualified ID: "crate_name::group_name"
    pub id: &'static str,
    /// Group identifier for item matching
    pub name: &'static str,
    /// Display label in menu bar
    pub label: &'static str,
    /// Ordering priority (lower = leftmost)
    pub priority: i16,
    /// Source crate
    pub source: RegistrySource,
}

/// Definition of a menu item within a group.
pub struct MenuItemDef {
    /// Full qualified ID: "crate_name::item_name"
    pub id: &'static str,
    /// Item identifier
    pub name: &'static str,
    /// Parent group name (matches MenuGroupDef.name)
    pub group: &'static str,
    /// Display label in dropdown
    pub label: &'static str,
    /// Command to execute when selected
    pub command: &'static str,
    /// Optional keyboard shortcut hint (display only)
    pub shortcut: Option<&'static str>,
    /// Ordering priority within group (lower = higher in menu)
    pub priority: i16,
    /// Source crate
    pub source: RegistrySource,
}
```

### Macro Syntax

```rust
// menu_group! macro
menu_group!(file, {
    label: "File",
    priority: 0,
});

// menu_item! macro
menu_item!(file_save, {
    group: "file",
    label: "Save",
    command: "write",
    shortcut: "ctrl-s",  // optional
    priority: 20,
});
```

### Generated Code Pattern

Following the `action!` macro pattern:

```rust
// menu_group! expands to:
paste::paste! {
    #[allow(non_upper_case_globals)]
    #[linkme::distributed_slice($crate::menu::MENU_GROUPS)]
    static [<MENU_GROUP_ $name>]: $crate::menu::MenuGroupDef = $crate::menu::MenuGroupDef {
        id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
        name: stringify!($name),
        label: $label,
        priority: $priority,
        source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
    };
}

// menu_item! expands to:
paste::paste! {
    #[allow(non_upper_case_globals)]
    #[linkme::distributed_slice($crate::menu::MENU_ITEMS)]
    static [<MENU_ITEM_ $name>]: $crate::menu::MenuItemDef = $crate::menu::MenuItemDef {
        id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
        name: stringify!($name),
        group: $group,
        label: $label,
        command: $command,
        shortcut: $crate::__opt!($({Some($shortcut)})?, None),
        priority: $crate::__opt!($({$priority})?, 50),
        source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
    };
}
```

### Menu Assembly

```rust
// crates/api/src/menu.rs

pub fn create_menu() -> MenuState<MenuAction> {
    use xeno_manifest::menu::{MENU_GROUPS, MENU_ITEMS};

    // Sort groups by priority
    let mut groups: Vec<_> = MENU_GROUPS.iter().collect();
    groups.sort_by_key(|g| g.priority);

    // Build menu structure
    let menu_items: Vec<MenuItem<MenuAction>> = groups
        .into_iter()
        .map(|group| {
            // Collect items for this group, sorted by priority
            let mut items: Vec<_> = MENU_ITEMS
                .iter()
                .filter(|item| item.group == group.name)
                .collect();
            items.sort_by_key(|i| i.priority);

            // Convert to MenuItem
            let children: Vec<MenuItem<MenuAction>> = items
                .into_iter()
                .map(|item| MenuItem::item(item.label, MenuAction::Command(item.command)))
                .collect();

            MenuItem::group(group.label, children)
        })
        .collect();

    MenuState::new(menu_items)
}
```

______________________________________________________________________

## Directory Structure

```
crates/
├── manifest/
│   └── src/
│       ├── lib.rs              # Add: pub mod menu; re-exports
│       ├── menu.rs             # NEW: MenuGroupDef, MenuItemDef, slices
│       └── macros/
│           ├── mod.rs          # Add: mod menu;
│           └── menu.rs         # NEW: menu_group!, menu_item! macros
├── stdlib/
│   └── src/
│       ├── lib.rs              # Add: mod menus;
│       └── menus/
│           ├── mod.rs          # NEW: module organization
│           ├── file.rs         # NEW: File menu group + items
│           ├── edit.rs         # NEW: Edit menu group + items
│           ├── view.rs         # NEW: View menu group + items
│           └── help.rs         # NEW: Help menu group + items
└── api/
    └── src/
        └── menu.rs             # MODIFY: build from slices
```

______________________________________________________________________

## Anti-Patterns

1. **Hardcoding menu structure**: Use distributed slices, not manual Vec construction
1. **Tight coupling**: Menu items should reference commands by name, not function pointers
1. **Ignoring existing patterns**: Study `action!`, `command!` macros before implementing
1. **Monolithic files**: Split menu definitions by group (file.rs, edit.rs, etc.)
1. **Missing priority system**: Always include priority for predictable ordering

______________________________________________________________________

## Success Criteria

1. `cargo build --workspace` succeeds
1. `cargo test --workspace` passes
1. Menu displays identically to current behavior
1. Extensions can add menu items by using `menu_group!` and `menu_item!` macros
1. Code follows existing patterns (compare with `action!`, `command!` implementations)

______________________________________________________________________

## Reference Files

Study these files for patterns:

- `crates/manifest/src/macros/actions.rs` - `action!` macro pattern
- `crates/manifest/src/macros/registry.rs` - `command!`, `motion!` patterns
- `crates/manifest/src/lib.rs` - slice declarations and exports
- `crates/stdlib/src/actions/` - distributed action definitions
- `crates/api/src/menu.rs` - current menu implementation (to be modified)
