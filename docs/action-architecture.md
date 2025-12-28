# Action Architecture

This document describes the action system architecture in Evildoer, the decisions behind it, and guidelines for implementing new actions.

## Overview

Evildoer uses a two-phase action system:

1. **Action Phase**: A pure function computes what should happen based on current state
2. **Handler Phase**: The result is applied to the editor

```
ActionContext ──> Action ──> ActionResult ──> Handler ──> Editor State
     (read)       (pure)       (data)        (apply)       (mutate)
```

This separation enables:
- Testable, pure action logic
- Reusable generic handlers
- Clear capability boundaries

## ActionResult Categories

### Parameterized Variants (shared handlers)

These carry computed data and share generic handlers:

| Variant | Data | Handler Behavior |
|---------|------|------------------|
| `Motion(Selection)` | New selection state | Sets selection and cursor |
| `Edit(EditAction)` | Edit operation enum | Delegates to edit system |
| `ModeChange(ActionMode)` | Target mode | Sets editor mode |
| `InsertWithMotion(Selection)` | Selection for insert | Sets selection, enters insert mode |
| `Pending(PendingAction)` | Pending state info | Enters pending mode |
| `Error(String)` | Error message | Displays error notification |
| `SearchNext { add_selection }` | Search params | Executes search via SearchAccess |
| `SearchPrev { add_selection }` | Search params | Executes search via SearchAccess |

**Key insight**: Many actions share these handlers. For example, all selection-manipulating actions return `Motion(Selection)` and reuse the same handler.

### Unit Variants (dedicated handlers)

These represent operations requiring external capabilities:

| Variant | Capability Required | Purpose |
|---------|---------------------|---------|
| `SplitHorizontal` | `BufferOpsAccess` | Create horizontal split |
| `SplitVertical` | `BufferOpsAccess` | Create vertical split |
| `FocusLeft/Right/Up/Down` | `BufferOpsAccess` | Navigate splits |
| `BufferNext/Prev` | `BufferOpsAccess` | Cycle buffers |
| `CloseBuffer` | `BufferOpsAccess` | Close current buffer |
| `UseSelectionAsSearch` | `SearchAccess` | Set search pattern |
| `Ok` | None | No-op success |
| `Quit/ForceQuit` | None | Exit editor |
| `ForceRedraw` | None | Trigger redraw |

### Stub Variants (awaiting implementation)

These exist for keybinding completeness but warn on use:
- `Align`, `CopyIndent`, `TabsToSpaces`, `SpacesToTabs`, `TrimSelections`

## Design Principle: Computation in Actions, Not Handlers

**Rule**: If an action can compute its result using only `ActionContext`, the computation belongs in the action function, not the handler.

`ActionContext` provides:
- `text: RopeSlice` - Document content (read-only)
- `selection: &Selection` - Current selection state
- `cursor: CharIdx` - Current cursor position
- `count: usize` - Numeric prefix (e.g., `3w`)
- `extend: bool` - Whether shift is held
- `register: Option<char>` - Named register
- `args: ActionArgs` - Additional arguments (char, string)

### Good: Pure Action Function

```rust
action!(
    merge_selections,
    { description: "Merge overlapping selections" },
    handler: merge_selections
);

fn merge_selections(ctx: &ActionContext) -> ActionResult {
    let mut new_sel = ctx.selection.clone();
    new_sel.merge_overlaps_and_adjacent();
    ActionResult::Motion(new_sel)
}
```

The action is a pure function: `ActionContext -> ActionResult`. Testing is straightforward.

### Bad: Complex Handler

```rust
// DON'T DO THIS
action!(merge_selections, result: ActionResult::MergeSelections);

// In handler file:
result_handler!(RESULT_MERGE_SELECTIONS_HANDLERS, ..., |r, ctx, _| {
    let mut sel = ctx.selection().clone();
    sel.merge_overlaps_and_adjacent();
    ctx.set_selection(sel);
    ctx.set_cursor(sel.primary().head);
    HandleOutcome::Handled
});
```

This spreads logic across files, requires a custom ActionResult variant, and makes testing harder.

### When Handlers ARE Needed

Use dedicated handlers when the operation requires capabilities beyond `ActionContext`:

```rust
// Search requires SearchAccess capability
result_handler!(RESULT_USE_SELECTION_SEARCH_HANDLERS, ..., |_, ctx, _| {
    if let Some(search) = ctx.search() {
        search.use_selection_as_pattern();
    }
    HandleOutcome::Handled
});

// Buffer operations require BufferOpsAccess
window_action!(
    split_horizontal,
    key: Key::char('s'),
    description: "Split horizontally",
    result: SplitHorizontal => RESULT_SPLIT_HORIZONTAL_HANDLERS,
    handler: |ops| ops.split_horizontal()
);
```

## Code Organization

### Action Files (`crates/stdlib/src/actions/`)

| File | Purpose |
|------|---------|
| `selection_ops.rs` | Selection manipulation (collapse, flip, split, duplicate, merge) |
| `motions.rs` | Cursor movement (word, line, document) |
| `editing.rs` | Text modification (delete, yank, paste, case) |
| `scroll.rs` | View scrolling |
| `insert.rs` | Insert mode entry points |
| `find.rs` | Character find (f, t, F, T) |
| `text_objects.rs` | Text object selection |
| `modes.rs` | Mode transitions |
| `misc.rs` | Uncategorized actions |

### Handler Files (`crates/stdlib/src/editor_ctx/result_handlers/`)

| File | Purpose |
|------|---------|
| `core.rs` | Generic handlers (Motion, ModeChange, Pending, Error, Quit) |
| `edit.rs` | EditAction dispatch |
| `mode.rs` | Mode change handling |
| `search.rs` | Search operations |
| `stubs.rs` | Unimplemented feature warnings |

### Colocated Actions (`crates/stdlib/src/window_actions.rs`)

Window mode actions use a macro that colocates:
- Action definition
- Keybinding registration
- Handler registration

```rust
window_action!(
    focus_left,
    key: Key::char('h'),
    description: "Focus split to the left",
    result: FocusLeft => RESULT_FOCUS_LEFT_HANDLERS,
    handler: |ops| ops.focus_left()
);
```

This pattern works well for imperative actions that:
1. Require a specific capability (BufferOpsAccess)
2. Have a 1:1 mapping between ActionResult variant and method call
3. Are bound to a specific mode's keys

## Capability System

Capabilities are traits that provide optional editor features:

| Trait | Purpose |
|-------|---------|
| `CursorAccess` | Get/set cursor position |
| `SelectionAccess` | Get/set selection state |
| `TextAccess` | Read document content |
| `ModeAccess` | Get/set editor mode |
| `MessageAccess` | Display notifications |
| `EditAccess` | Text modification operations |
| `SearchAccess` | Pattern search |
| `UndoAccess` | Undo/redo history |
| `BufferOpsAccess` | Split/buffer management |
| `FileOpsAccess` | Save/load operations |
| `ThemeAccess` | Theme switching |

The first five are required; others are optional and checked at runtime.

## Adding New Actions

### Step 1: Determine the Result Type

Ask: "Can this be computed purely from ActionContext?"

- **Yes**: Return `ActionResult::Motion(new_selection)` or appropriate parameterized variant
- **No**: You need a capability. Check if an existing one fits, or if a new one is warranted.

### Step 2: Implement the Action

For pure actions:

```rust
action!(
    my_action,
    { description: "Does something useful" },
    handler: my_action
);

fn my_action(ctx: &ActionContext) -> ActionResult {
    // Compute new state from ctx
    ActionResult::Motion(new_selection)
}
```

For capability-requiring actions, add to the appropriate colocated file or create a handler.

### Step 3: Add Keybinding

In the appropriate keybindings file (`normal.rs`, `insert.rs`, etc.):

```rust
bind!(KB_MY_ACTION, Key::char('x'), "my_action");
```

Or use the colocated macro if applicable.

## Handler Infrastructure

### `#[derive(DispatchResult)]`

The `ActionResult` enum uses `#[derive(DispatchResult)]` (from `evildoer-macro`) to auto-generate dispatch infrastructure:

```rust
#[derive(Debug, Clone, DispatchResult)]
pub enum ActionResult {
    #[terminal_safe]
    Ok,
    #[terminal_safe]
    #[handler(Quit)]  // Share handler with ForceQuit
    Quit,
    #[terminal_safe]
    #[handler(Quit)]
    ForceQuit,
    Motion(Selection),
    // ...
}
```

This generates:
- **Handler slices**: `RESULT_OK_HANDLERS`, `RESULT_QUIT_HANDLERS`, `RESULT_MOTION_HANDLERS`, etc.
- **`dispatch_result` function**: Routes results to their handler slices
- **`is_terminal_safe` method**: Returns `true` for variants marked `#[terminal_safe]`

#### Attributes

| Attribute | Purpose |
|-----------|---------|
| `#[terminal_safe]` | Marks variant as safe when terminal is focused (workspace-level ops) |
| `#[handler(Name)]` | Override handler slice name (e.g., `Quit` and `ForceQuit` share `RESULT_QUIT_HANDLERS`) |

#### Adding New Variants

1. Add the variant to `ActionResult`
2. If terminal-safe, add `#[terminal_safe]`
3. If sharing a handler, add `#[handler(ExistingVariant)]`
4. Register a handler with `result_handler!` macro

The handler slice is auto-generated; no manual `distributed_slice` declaration needed.

### Handler Registration

Handlers are registered to the generated slices:

```rust
result_handler!(RESULT_OK_HANDLERS, HANDLE_OK, "ok", |_, _, _| {
    HandleOutcome::Handled
});

result_handler!(RESULT_MOTION_HANDLERS, HANDLE_MOTION, "motion", |result, ctx, extend| {
    if let ActionResult::Motion(sel) = result {
        ctx.set_selection(sel.clone());
        ctx.set_cursor(sel.primary().head);
    }
    HandleOutcome::Handled
});
```

Handler slices are in `evildoer_manifest::actions::*`, not `evildoer_manifest::editor_ctx::*`.

## Historical Decisions

### Removing Orphan Variants

We removed ActionResult variants that had no corresponding action or handler:
- `JumpForward`, `JumpBackward`, `SaveJump`
- `RecordMacro`, `PlayMacro`
- `SaveSelections`, `RestoreSelections`
- `RepeatLastInsert`, `RepeatLastObject`

These represented planned features with no implementation. They can be re-added when implemented.

### Removing SelectionOpsAccess

The `SelectionOpsAccess` capability trait was removed because its methods (`split_lines`, `merge_selections`) could be implemented as pure actions returning `Motion(Selection)`. This eliminated:
- The capability trait and enum variant
- Two handler slices
- Implementation boilerplate in Editor

### Moving Complex Handlers to Actions

`duplicate_selections_down`, `duplicate_selections_up`, and `merge_selections` were refactored from:
- Unit ActionResult variant + 40-line handler

To:
- Pure action function returning `Motion(Selection)`

This reduced code, improved testability, and eliminated three ActionResult variants.

### Auto-Generated Dispatch

Previously, `dispatch_result` and handler slices were manually maintained in `handlers.rs` (~100 lines of boilerplate). Each new `ActionResult` variant required:
1. Adding a `result_slices!` entry
2. Adding a match arm in `dispatch_result`
3. Updating `is_terminal_safe` if applicable

Now, `#[derive(DispatchResult)]` generates all of this. Adding a new variant only requires adding the variant itself (with optional attributes).

## Metrics

After refactoring:

- **30 handler slices** (infrastructure + stubs + window actions)
- **31 ActionResult variants** (down from 44)
- **Pure action functions** for all selection operations

The goal is to minimize custom ActionResult variants and handlers, preferring pure action functions that return parameterized variants like `Motion(Selection)`.
