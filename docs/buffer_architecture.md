# Buffer Architecture Refactor

## Current State

The Editor (Workspace) now supports multiple buffers with split views.

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

### Phase 2: Multi-Buffer Support ✓ COMPLETE

- [x] Editor uses `HashMap<BufferId, Buffer>` for multiple buffers
- [x] Added `Layout` enum for split view management (`buffer/layout.rs`)
- [x] Window mode (`Ctrl+w`) with navigation keybindings
- [x] Buffer commands: `split_horizontal`, `split_vertical`, `buffer_next`, `buffer_prev`, `close_buffer`
- [x] Status line shows buffer count `[1/2]` when multiple buffers open

### Phase 3: Split Rendering ✓ COMPLETE

- [x] Created `BufferRenderContext` for buffer-agnostic rendering
- [x] `render_split_buffers()` iterates layout and renders each buffer
- [x] `ensure_buffer_cursor_visible()` works per-buffer
- [x] Separator lines rendered between splits (│ for horizontal, ─ for vertical)
- [x] Each buffer renders independently with its own cursor and selection
- [x] Only focused buffer shows active cursor style

### Cleanup Status

Files refactored to delegate to Buffer:

- [x] `editor/history.rs` - delegates to `buffer.undo()` / `buffer.redo()` with notification handling
- [x] `editor/navigation.rs` - delegates to buffer navigation methods
- [x] `render/buffer_render.rs` - new module with `BufferRenderContext` for split rendering
- [x] `render/document.rs` - uses `render_split_buffers()` for layout-aware rendering
- [ ] `editor/actions.rs` - still directly accesses `self.buffer.X`, could move logic to Buffer
- [ ] `editor/actions_exec.rs` - action execution, stays in Editor for now
- [ ] `editor/search.rs` - could move to Buffer (returns Result, Editor handles notify)

Editor struct is now a thin wrapper around Buffer:

- [x] Buffer-specific fields moved to `Buffer` struct
- [x] Editor accesses buffer state via `self.buffer()` / `self.buffer_mut()`
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

## Key Keybindings

Window mode is activated with `Ctrl+w`:

- `s` - Split horizontal (side by side)
- `v` - Split vertical (stacked)
- `h/j/k/l` - Focus left/down/up/right (future: directional navigation)
- `n` - Next buffer
- `p` - Previous buffer
- `q/c` - Close current buffer
- `o` - Close other buffers

## Rendering Architecture

The split rendering uses a context-based approach:

```rust
// BufferRenderContext holds shared resources
pub struct BufferRenderContext<'a> {
    pub theme: &'static Theme,
    pub language_loader: &'a LanguageLoader,
    pub style_overlays: &'a StyleOverlays,
}

// render_buffer can render any buffer given its area
impl BufferRenderContext<'_> {
    pub fn render_buffer(&self, buffer: &Buffer, area: Rect, use_block_cursor: bool) -> RenderResult;
}
```

The main `render()` method:

1. Computes areas for all buffers via `layout.compute_areas(doc_area)`
2. Ensures cursor visibility for each buffer in its area
3. Creates `BufferRenderContext` with shared resources
4. Renders each buffer independently
5. Draws separator lines between splits

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
