# Evildoer

The next evolution of agentic text editors & harnesses.

## Design Goals

- **Orthogonal**: No tight coupling between modules. Event emitter/receiver pattern via `linkme` distributed slices for compile-time registration.
- **Suckless extension system**: Extensions written in Rust. Two-tier system: Core Builtins (stdlib) + Host Extensions (extensions/).
- **Data-driven macros**: Declarative and proc macros keep registration patterns lean and composable.

## Crate Architecture

```
evildoer-base          Core types: Range, Selection, Transaction, Rope wrappers
evildoer-manifest      Registry DEFINITIONS (ActionDef, CommandDef, etc.) - no implementations
evildoer-stdlib        Registry IMPLEMENTATIONS (actions, commands, motions, etc.)
evildoer-macro         Proc macros (DispatchResult, define_events!, parse_keybindings, etc.)
evildoer-api           Editor engine: Buffer, Editor, rendering, terminals
evildoer-extensions    Host extensions discovered at build-time (LSP, Zenmode)
evildoer-acp           AI completion protocol integration (experimental)
evildoer-term          Main binary and terminal UI
```

**Supporting crates**: `keymap` (key parsing), `input` (input state machine), `config` (KDL parsing), `language` (tree-sitter), `lsp` (LSP client framework), `tui` (Ratatui fork).

## Registry System

Uses `linkme` distributed slices for compile-time registration. Definitions live in **evildoer-manifest**, implementations in **evildoer-stdlib**.

| Registry      | Slice                       | Implementations                        | Macro                    |
| ------------- | --------------------------- | -------------------------------------- | ------------------------ |
| Actions       | `ACTIONS`                   | `stdlib/src/actions/` (87 items)       | `action!`                |
| Commands      | `COMMANDS`                  | `stdlib/src/commands/` (19 items)      | `command!`               |
| Motions       | `MOTIONS`                   | `stdlib/src/motions/` (19 items)       | `motion!`                |
| Text Objects  | `TEXT_OBJECTS`              | `stdlib/src/objects/` (9 items)        | `text_object!`           |
| Options       | `OPTIONS`                   | `stdlib/src/options/` (26 items)       | `option!`                |
| Hooks         | `HOOKS`                     | `stdlib/src/hooks/`                    | `hook!`, `async_hook!`   |
| Events        | (generated enums)           | `manifest/src/hooks.rs`                | `define_events!`         |
| Statusline    | `STATUSLINE_SEGMENTS`       | `stdlib/src/statusline/` (6 items)     | `statusline_segment!`    |
| Notifications | `NOTIFICATION_TYPES`        | `stdlib/src/notifications/` (5 types)  | `register_notification!` |
| Panels        | `PANELS`, `PANEL_FACTORIES` | `api/src/panels/` (2 items)            | `panel!`                 |
| Keybindings   | `KEYBINDINGS`               | Colocated with actions via `bindings:` | (inline in `action!`)    |

### Action Result Dispatch

Actions return `ActionResult` variants which are dispatched to handlers via `#[derive(DispatchResult)]`:

```rust
action!(move_left, {
    description: "Move cursor left",
    bindings: r#"normal "h" "left""#,
}, |ctx| cursor_motion(ctx, "char_prev"));
```

Handler slices (`RESULT_*_HANDLERS`) are auto-generated. Extensions can add handlers for existing result types via the `RESULT_EXTENSION_HANDLERS` distributed slice:

```rust
result_extension_handler!(my_handler, |result, ctx| {
    // Runs after core handlers for any result type
});
```

### Event System

Hook events are defined via the `define_events!` proc macro in `manifest/src/hooks.rs`. This is a **single source of truth** that generates:

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
- `PanelToggled` - emitted when panels open/close

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

Fine-grained traits in `manifest/src/editor_ctx/capabilities.rs`:

| Trait             | Required | Purpose                 |
| ----------------- | -------- | ----------------------- |
| `CursorAccess`    | Yes      | Get/set cursor position |
| `SelectionAccess` | Yes      | Get/set selections      |
| `ModeAccess`      | Yes      | Get/set editor mode     |
| `MessageAccess`   | Yes      | Display notifications   |
| `EditAccess`      | Optional | Text modifications      |
| `SearchAccess`    | Optional | Pattern search          |
| `UndoAccess`      | Optional | Undo/redo history       |
| `SplitOps`        | Optional | Split management        |
| `PanelOps`        | Optional | Panel management        |
| `FocusOps`        | Optional | Focus/buffer navigation |
| `FileOpsAccess`   | Optional | Save/load operations    |
| `JumpAccess`      | Optional | Jump list navigation    |
| `MacroAccess`     | Optional | Macro recording/playback|

**Pending traits**: `TextAccess` (read-only document access for result handlers).

## Key Files

| Purpose             | Location                                        |
| ------------------- | ----------------------------------------------- |
| Main entry          | `crates/term/src/main.rs`                       |
| Editor core         | `crates/api/src/editor/mod.rs`                  |
| Action definitions  | `crates/manifest/src/actions/`                  |
| Motion definitions  | `crates/manifest/src/motions.rs`                |
| Text objects        | `crates/manifest/src/text_objects.rs`           |
| Hook events         | `crates/manifest/src/hooks.rs`                  |
| Declarative macros  | `crates/manifest/src/macros/`                   |
| Proc macros         | `crates/macro/src/lib.rs`                       |
| Event proc macro    | `crates/macro/src/events.rs`                    |
| Registry infra      | `crates/manifest/src/registry/`                 |
| Result handlers     | `crates/stdlib/src/editor_ctx/result_handlers/` |
| Keymap registry     | `crates/manifest/src/keymap_registry.rs`        |
| Extension discovery | `crates/extensions/build.rs`                    |

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
