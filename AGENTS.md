# Xeno

Modal text editor with compile-time registry system.

## Crates

```
xeno-base          Core types: Range, Selection, Rope wrappers
xeno-registry      Registry definitions (actions/, commands/, motions/, etc.)
xeno-registry-core Shared primitives (Key, RegistryMetadata)
xeno-core          Glue: ActionId, KeymapRegistry, movement
xeno-macro         Proc macros (DispatchResult, define_events!, parse_keybindings)
xeno-api           Editor engine: Buffer, Editor, rendering
xeno-extensions    Host extensions (LSP, Zenmode)
xeno-term          Main binary
```

**Supporting**: `keymap`, `input`, `config`, `language`, `lsp`, `tui`.

## Registry System

Uses `linkme` distributed slices for compile-time registration. Each registry under `crates/registry/`.

| Registry      | Slice                 | Macro                  |
| ------------- | --------------------- | ---------------------- |
| Actions       | `ACTIONS`             | `action!`              |
| Commands      | `COMMANDS`            | `command!`             |
| Motions       | `MOTIONS`             | `motion!`              |
| Text Objects  | `TEXT_OBJECTS`        | `text_object!`         |
| Options       | `OPTIONS`             | `option!`              |
| Hooks         | `HOOKS`               | `hook!`, `async_hook!` |
| Statusline    | `STATUSLINE_SEGMENTS` | `statusline_segment!`  |
| Gutter        | `GUTTERS`             | `gutter!`              |
| Notifications | `NOTIFICATION_TYPES`  | -                      |
| Themes        | `THEMES`              | -                      |
| Keybindings   | `KEYBINDINGS`         | (inline in `action!`)  |

### Typed Handles

Typed keys (`*::keys::*`) for compile-time safety. Strings only at boundaries (user input, config).

```rust
use xeno_registry_motions::keys as motions;

action!(move_left, {
    description: "Move cursor left",
    bindings: r#"normal "h" "left""#,
}, |ctx| cursor_motion(ctx, motions::left));
```

Handler slices (`RESULT_*_HANDLERS`) auto-generated via `#[derive(DispatchResult)]`.

### Event System

Hook events are defined via the `define_events!` proc macro in `registry/hooks/src/events.rs`. This is a **single source of truth** that generates:

- `HookEvent` enum for event discrimination
- `HookEventData<'a>` with borrowed payloads for sync hooks
- `OwnedHookContext` with owned payloads for async hooks
- `__hook_extract!` and `__async_hook_extract!` macros for parameter binding

```rust
define_events! {
    BufferOpen => "buffer:open" {
        path: Path,
        text: RopeSlice,
        file_type: OptionStr,
    },
    EditorQuit => "editor:quit",  // Unit events have no payload
}
```

Field type tokens: `Path` → `&Path`/`PathBuf`, `RopeSlice` → `RopeSlice<'a>`/`String`, `OptionStr` → `Option<&str>`/`Option<String>`, `ViewId`/`Bool` → copy types.

**Action Lifecycle Events**: `ActionPre` (before execution), `ActionPost` (after result dispatch).

## Extension System

Extensions in `crates/extensions/extensions/` are discovered at build-time via `build.rs`.

**Current extensions**: LSP (`extensions/lsp/`), Zenmode (`extensions/zenmode/`)

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

**ACP** (`crates/acp/`): Separate crate for AI completion, loaded via `use xeno_acp as _` in main.

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
KITTY_TESTS=1 DISPLAY=:0 nix develop -c cargo test -p xeno-term --test kitty_multiselect -- --nocapture --test-threads=1
```

### Testing Philosophy

- Unit tests for core logic (selections, motions, text objects)
- Integration tests via `kitty-test-harness` for GUI behavior
- Write failing assertions first, iterate until green
- GUI harness catches cursor/selection drift that unit tests miss

# DEV NOTES

<!-- Below this line, agents to add important operational details or unintuative notes that new developers should know. -->

