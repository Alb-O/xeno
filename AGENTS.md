# Evildoer

The next evolution of agentic text editors & harnesses.

## Design Goals

- **Orthogonal**: No tight coupling between modules. Event emitter/receiver pattern via `linkme` distributed slices for compile-time registration.
- **Suckless extension system**: Extensions written in Rust. Two-tier system: Core Builtins + Host Extensions (extensions/).
- **Data-driven macros**: Declarative and proc macros keep registration patterns lean and composable.

## Crate Architecture

```
evildoer-base          Core types: Range, Selection, Transaction, Rope wrappers
evildoer-registry      Registry definitions organized by type (actions/, commands/, etc.)
evildoer-registry-core Shared registry primitives (Key, RegistryMetadata)
evildoer-core          Glue layer: ActionId, KeymapRegistry, movement, notifications
evildoer-macro         Proc macros (DispatchResult, define_events!, parse_keybindings, etc.)
evildoer-api           Editor engine: Buffer, Editor, rendering, terminals
evildoer-extensions    Host extensions discovered at build-time (LSP, Zenmode)
evildoer-acp           AI completion protocol integration (experimental)
evildoer-term          Main binary and terminal UI
```

**Supporting crates**: `keymap` (key parsing), `input` (input state machine), `config` (KDL parsing), `language` (tree-sitter), `lsp` (LSP client framework), `tui` (Ratatui fork).

## Registry System

Uses `linkme` distributed slices for compile-time registration. Each registry is a self-contained crate under `crates/registry/`.

| Registry      | Crate                             | Slice                 | Macro                    |
| ------------- | --------------------------------- | --------------------- | ------------------------ |
| Actions       | `evildoer-registry-actions`       | `ACTIONS`             | `action!`                |
| Commands      | `evildoer-registry-commands`      | `COMMANDS`            | `command!`               |
| Motions       | `evildoer-registry-motions`       | `MOTIONS`             | `motion!`                |
| Text Objects  | `evildoer-registry-text-objects`  | `TEXT_OBJECTS`        | `text_object!`           |
| Options       | `evildoer-registry-options`       | `OPTIONS`             | `option!`                |
| Hooks         | `evildoer-registry-hooks`         | `HOOKS`               | `hook!`, `async_hook!`   |
| Statusline    | `evildoer-registry-statusline`    | `STATUSLINE_SEGMENTS` | `statusline_segment!`    |
| Notifications | `evildoer-registry-notifications` | `NOTIFICATION_TYPES`  | `register_notification!` |
| Themes        | `evildoer-registry-themes`        | `THEMES`              | -                        |
| Menus         | `evildoer-registry-menus`         | `MENUS`               | -                        |
| Keybindings   | (in evildoer-registry)            | `KEYBINDINGS`         | (inline in `action!`)    |

### Typed Handles

Typed handles provide compile-time safety for internal registry references:

- Motions: `evildoer_registry_motions::keys::*` used with `cursor_motion` helpers
- Actions: `evildoer_registry_actions::keys::*` used for hardcoded action IDs
- Strings remain at boundaries (user input, config, runtime lookup)

### Action Result Dispatch

Actions return `ActionResult` variants which are dispatched to handlers via `#[derive(DispatchResult)]`:

```rust
use evildoer_registry_motions::keys as motions;

action!(move_left, {
    description: "Move cursor left",
    bindings: r#"normal "h" "left""#,
}, |ctx| cursor_motion(ctx, motions::left));
```

Handler slices (`RESULT_*_HANDLERS`) are auto-generated. Extensions can add handlers for existing result types via the `RESULT_EXTENSION_HANDLERS` distributed slice:

```rust
result_extension_handler!(my_handler, |result, ctx| {
    // Runs after core handlers for any result type
});
```

### Event System

Hook events are defined via the `define_events!` proc macro in `registry/hooks/src/events.rs`. This is a **single source of truth** that generates:

- `HookEvent` enum for event discrimination
- `HookEventData<'a>` with borrowed payloads for sync hooks
- `OwnedHookContext` with owned payloads for async hooks
- `__hook_extract!` and `__async_hook_extract!` macros for parameter binding

```rust
// Adding a new event is one line:
define_events! {
    BufferOpen => "buffer:open" {
        path: Path,
        text: RopeSlice,
        file_type: OptionStr,
    },
    EditorQuit => "editor:quit",  // Unit events have no payload
}
```

Field type tokens are mapped automatically:

- `Path` → `&Path` / `PathBuf`
- `RopeSlice` → `RopeSlice<'a>` / `String`
- `OptionStr` → `Option<&str>` / `Option<String>`
- `ViewId` → `ViewId` (copy type)
- `Bool` → `bool`

**Focus & Layout Events** (observable via hooks):

- `ViewFocusChanged` - emitted when focus changes between views
- `SplitCreated` / `SplitClosed` - emitted on split operations

**Action Lifecycle Events**:

- `ActionPre` - emitted before action execution
- `ActionPost` - emitted after result dispatch with result variant name

## Extension System

Extensions in `crates/extensions/extensions/` are discovered at build-time via `build.rs`.

**Current extensions**:

- **LSP** (`extensions/lsp/`): Language server integration via `async_hook!` macros
- **Zenmode** (`extensions/zenmode/`): Focus mode with style overlays, uses `#[extension]` macro

**Extension macro** supports `#[init]`, `#[render]`, `#[command]` attributes:

```rust
#[extension(id = "zenmode", priority = 100)]
impl ZenmodeState {
    #[init]
    pub fn new() -> Self { ... }

    #[render(priority = 100)]
    fn update(&mut self, editor: &mut Editor) { ... }

    #[command("zenmode", aliases = ["zen", "focus"])]
    fn toggle(&mut self, ctx: &mut CommandContext) -> CommandResult { ... }
}
```

**ACP** (`crates/acp/`): Separate crate for AI completion, loaded via `use evildoer_acp as _` in main.

## Capability System

Fine-grained traits in `registry/actions/src/editor_ctx/capabilities.rs`:

| Trait             | Required | Purpose                  |
| ----------------- | -------- | ------------------------ |
| `CursorAccess`    | Yes      | Get/set cursor position  |
| `SelectionAccess` | Yes      | Get/set selections       |
| `ModeAccess`      | Yes      | Get/set editor mode      |
| `MessageAccess`   | Yes      | Display notifications    |
| `EditAccess`      | Optional | Text modifications       |
| `SearchAccess`    | Optional | Pattern search           |
| `UndoAccess`      | Optional | Undo/redo history        |
| `SplitOps`        | Optional | Split management         |
| `FocusOps`        | Optional | Focus/buffer navigation  |
| `ViewportAccess`  | Optional | Viewport queries         |
| `FileOpsAccess`   | Optional | Save/load operations     |
| `JumpAccess`      | Optional | Jump list navigation     |
| `MacroAccess`     | Optional | Macro recording/playback |

**Pending traits**: `TextAccess` (read-only document access for result handlers).

## Key Files

| Purpose             | Location                                      |
| ------------------- | --------------------------------------------- |
| Main entry          | `crates/term/src/main.rs`                     |
| Editor core         | `crates/api/src/editor/mod.rs`                |
| Action definitions  | `crates/registry/actions/src/`                |
| Motion definitions  | `crates/registry/motions/src/`                |
| Text objects        | `crates/registry/text_objects/src/`           |
| Hook events         | `crates/registry/hooks/src/events.rs`         |
| Proc macros         | `crates/macro/src/lib.rs`                     |
| Event proc macro    | `crates/macro/src/events.rs`                  |
| Result handlers     | `crates/core/src/editor_ctx/result_handlers/` |
| Keymap registry     | `crates/core/src/keymap_registry.rs`          |
| Movement functions  | `crates/core/src/movement/`                   |
| Extension discovery | `crates/extensions/build.rs`                  |

## Development

```bash
# Build
nix develop -c cargo build

# Test
nix develop -c cargo test --workspace

# Kitty GUI tests
KITTY_TESTS=1 DISPLAY=:0 nix develop -c cargo test -p evildoer-term --test kitty_multiselect -- --nocapture --test-threads=1
```

### Testing Philosophy

- Unit tests for core logic (selections, motions, text objects)
- Integration tests via `kitty-test-harness` for GUI behavior
- Write failing assertions first, iterate until green
- GUI harness catches cursor/selection drift that unit tests miss
