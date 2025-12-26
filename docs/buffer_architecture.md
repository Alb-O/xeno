# Buffer Architecture Refactor

## Current State

The current `Editor` struct conflates two concerns:

1. **Buffer state**: text content, cursor, selection, undo/redo, syntax
1. **Application state**: theme, UI, notifications, extensions, filesystem

This makes it impossible to have multiple text buffers open simultaneously.

## Progress

### Phase 1: Extract Buffer ✓ COMPLETE

- [x] Create `Buffer` struct with buffer-specific fields
- [x] Create `buffer/mod.rs` with core Buffer struct
- [x] Create `buffer/history.rs` with undo/redo (returns `HistoryResult`)
- [x] Create `buffer/navigation.rs` with cursor movement
- [x] Extract `wrap_line` as standalone function in `render/types.rs`
- [x] Add `buffer/editing.rs` for text manipulation (insert, delete, transactions)
- [x] Editor holds single Buffer internally (API compatibility)
- [x] Update Editor methods to delegate to Buffer
- [x] Update all external call sites

### Current State

**Phase 1 is complete.** The Editor struct now contains a `Buffer` field and all
buffer-specific operations are delegated to it:

- `Editor.buffer: Buffer` - the active buffer
- Editor's history.rs delegates to Buffer's undo/redo with notification handling
- Editor's navigation.rs delegates to Buffer's movement methods
- All rendering and actions access buffer state via `self.buffer.X`
- Extensions updated to use `editor.buffer.X` pattern

The architecture is now ready for Phase 2 (multi-buffer support).

### Cleanup Status

Files refactored to delegate to Buffer:

- [x] `editor/history.rs` - now delegates to `buffer.undo()` / `buffer.redo()` with notification handling
- [x] `editor/navigation.rs` - now delegates to buffer navigation methods
- [ ] `editor/actions.rs` - still directly accesses `self.buffer.X`, could move logic to Buffer
- [ ] `editor/actions_exec.rs` - action execution, stays in Editor for now
- [ ] `editor/search.rs` - could move to Buffer (returns Result, Editor handles notify)
- [x] `render/document/wrapping.rs` - Editor impl delegates to `wrap_line()` standalone function

Editor struct is now a thin wrapper around Buffer:

- [x] Buffer-specific fields moved to `Buffer` struct
- [x] Editor accesses buffer state via `self.buffer.X`
- [x] Editor keeps workspace-level state (theme, ui, notifications, extensions, fs)

### Design Pattern for Buffer Methods

Buffer methods that need to communicate with the user should:

1. Return a `Result<T, BufferError>` or a message enum
1. Not call `notify()` directly (that's a Workspace concern)
1. The Workspace/Editor layer handles the Result and calls `notify()`

Example:

```rust
// In Buffer
pub fn undo(&mut self, lang: &LanguageLoader) -> Result<(), &'static str> {
    if let Some(entry) = self.undo_stack.pop() { ... Ok(()) }
    else { Err("Nothing to undo") }
}

// In Workspace/Editor  
pub fn undo(&mut self) {
    match self.buffer.undo(&self.language_loader) {
        Ok(()) => self.notify("info", "Undo"),
        Err(msg) => self.notify("warn", msg),
    }
}
```

## Target Architecture

### Buffer

The core text editing unit. Each buffer represents one file or scratch document.

```rust
pub struct Buffer {
    // Identity
    id: BufferId,
    
    // Content
    doc: Rope,
    
    // Cursor & Selection (multi-cursor support)
    selection: Selection,
    
    // Input handling (modal state)
    input: InputHandler,
    
    // File association
    path: Option<PathBuf>,
    modified: bool,
    
    // History
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    
    // View state (per-buffer, not per-view for now)
    scroll_line: usize,
    scroll_col: usize,
    
    // Syntax
    file_type: Option<String>,
    syntax: Option<Syntax>,
}
```

Key methods:

- `handle_key(&mut self, key: Key, ctx: &mut BufferContext) -> KeyResult`
- `apply_transaction(&mut self, tx: &Transaction)`
- `insert_text(&mut self, text: &str)`
- `undo(&mut self) / redo(&mut self)`

### BufferContext

Shared resources passed to buffer operations (not owned by buffer):

```rust
pub struct BufferContext<'a> {
    pub registers: &'a mut Registers,
    pub language_loader: &'a LanguageLoader,
    pub fs: &'a Arc<dyn FileSystem>,
    pub theme: &'a Theme,
}
```

### Workspace

The application-level container managing buffers and layout.

```rust
pub struct Workspace {
    // Buffer management
    buffers: HashMap<BufferId, Buffer>,
    next_buffer_id: u64,
    
    // Layout & focus
    layout: Layout,
    focused_buffer: BufferId,
    
    // Shared resources
    registers: Registers,
    theme: &'static Theme,
    language_loader: LanguageLoader,
    fs: Arc<dyn FileSystem>,
    
    // UI layer
    ui: UiManager,
    notifications: Notifications,
    extensions: ExtensionMap,
    
    // Window state
    window_width: Option<u16>,
    window_height: Option<u16>,
}
```

### Layout

Manages how buffers are displayed in splits.

```rust
pub enum Layout {
    Single(BufferId),
    Split {
        direction: Direction,  // Horizontal | Vertical
        ratio: f32,            // 0.0..1.0, position of split
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

pub enum Direction {
    Horizontal,  // side by side
    Vertical,    // stacked
}
```

## Migration Path

### Phase 1: Extract Buffer

1. Create `Buffer` struct with buffer-specific fields from `Editor`
1. `Editor` holds a single `Buffer` internally (maintains API compatibility)
1. Move buffer methods to `Buffer` impl

### Phase 2: Create Workspace

1. Rename `Editor` to `Workspace`
1. Change internal `Buffer` to `HashMap<BufferId, Buffer>`
1. Add `Layout` for split management
1. Update all call sites

### Phase 3: Split Commands

1. Add `:split` / `:vsplit` commands
1. Add `Ctrl+w` window navigation
1. Buffer switching (`:buffer`, `:bnext`, etc.)

## Key Decisions

### Selection Model (Kakoune-style)

Each buffer has a `Selection` which is a collection of `Range`s. This naturally supports:

- Single cursor (one range of length 0)
- Visual selection (one range)
- Multi-cursor (multiple ranges)

### Input Handler Per Buffer

Each buffer has its own `InputHandler` tracking mode (Normal/Insert/etc.). This allows:

- Different buffers in different modes
- Independent pending key state

### Scroll State

Initially, scroll state is per-buffer (not per-view). If we add multiple views of the same buffer, we'd need to extract this to a `View` struct.

### SplitBuffer Trait

The existing `SplitBuffer` trait is for non-text panels (terminal, file tree). These remain separate from `Buffer`. The dock system (`UiManager`) handles these.

Text splits are managed by `Layout`, not by `SplitBuffer`.
