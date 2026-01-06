# Task 10A: Extended LSP Capabilities with UI

## Model Directive

This document specifies the implementation of comprehensive LSP UI integration for the xeno editor. The goal is to surface all LSP features through appropriate UI primitives: popups for hover/completion/signature, overlays for diagnostics/inlay hints, and panels for references/symbols.

**Context**: The LSP infrastructure is solid (`xeno-lsp` crate handles JSON-RPC, client spawning, document sync). The high-level API exists in `xeno-api/src/lsp.rs` with methods for hover, completion, goto_definition, references, and format. Client capabilities already advertise support for signature help, inlay hints, code actions, and diagnostics. What's missing is the **UI layer** to surface these results.

**Scope**: Build popup infrastructure, wire diagnostics to display, implement completion menu, hover tooltip, signature help, code actions, and navigation features.

---

## Implementation Expectations

<mandatory_execution_requirements>

This is a **feature implementation** task requiring both infrastructure and integration work:

1. Build new UI components incrementally with tests
2. Wire LSP responses to UI after each component is functional
3. Run `cargo build --workspace` after structural changes
4. Run `cargo test --workspace` after each phase completion
5. Test manually with a real language server (rust-analyzer recommended)

Unacceptable:
- UI components that don't integrate with real LSP data
- Breaking existing editor functionality
- Popup/overlay systems that don't handle edge cases (screen edges, long content)
- Features that work only for specific languages

</mandatory_execution_requirements>

---

## Behavioral Constraints

<verbosity_and_scope_constraints>

- Extend existing patterns (Panel trait, style overlays) rather than inventing new systems
- Keep popup/overlay code in `xeno-api` crate, not scattered
- Follow existing theme color usage patterns (`theme.colors.popup.*`, `theme.colors.ui.*`)
- Prefer simple implementations first; optimize later
- Each phase should result in user-visible functionality

</verbosity_and_scope_constraints>

<design_freedom>

- Popup positioning algorithm can be custom-designed
- Completion filtering/sorting logic is flexible
- Diagnostic display style (underline type, virtual text format) is open
- Keybinding choices for new features are flexible
- Animation/transition effects are optional enhancements

</design_freedom>

---

## Current State Analysis

### LSP Infrastructure (Exists)

| Component | Location | Status |
|-----------|----------|--------|
| JSON-RPC client | `crates/lsp/src/client/mod.rs` | Complete |
| Document sync | `crates/lsp/src/sync.rs` | Complete |
| Diagnostics storage | `crates/lsp/src/document.rs` | Complete (not surfaced) |
| Position conversion | `crates/lsp/src/position.rs` | Complete |
| Client capabilities | `crates/lsp/src/client/capabilities.rs` | Advertises full support |

### High-Level API (Exists)

| Method | Location | Returns |
|--------|----------|---------|
| `LspManager::hover()` | `crates/api/src/lsp.rs:261` | `Option<Hover>` |
| `LspManager::completion()` | `crates/api/src/lsp.rs:281` | `Option<CompletionResponse>` |
| `LspManager::goto_definition()` | `crates/api/src/lsp.rs:304` | `Option<GotoDefinitionResponse>` |
| `LspManager::references()` | `crates/api/src/lsp.rs:327` | `Option<Vec<Location>>` |
| `LspManager::format()` | `crates/api/src/lsp.rs:351` | `Option<Vec<TextEdit>>` |
| `LspManager::get_diagnostics()` | `crates/api/src/lsp.rs:217` | `Vec<Diagnostic>` |

### UI Infrastructure (Exists)

| Component | Location | Status |
|-----------|----------|--------|
| Panel system | `crates/api/src/ui/panel.rs` | Complete |
| Dock manager | `crates/api/src/ui/dock.rs` | Complete |
| Focus routing | `crates/api/src/ui/focus.rs` | Complete |
| Style overlays | `crates/api/src/render/buffer/` | Exists (needs extension) |
| Gutter signs | `crates/registry/gutter/src/impls/signs.rs` | Has `diagnostic_severity` field |
| Notifications | `crates/tui/src/widgets/notifications/` | Toast system exists |
| Menu widget | `crates/tui/src/widgets/menu/` | Dropdown menus exist |

### Missing (The Gap)

- No popup/overlay manager for cursor-anchored UI
- Diagnostics tracked but not rendered (gutter, underlines, messages)
- No completion popup
- No hover tooltip
- No signature help display
- No code actions UI
- No navigation integration (goto def opens file, but no preview/picker)

---

## Implementation Roadmap

### Phase 1: Popup Infrastructure

**Objective**: Create reusable popup system for hover, completion, signature help, and code actions.

**Files**:
- `crates/api/src/ui/popup/mod.rs` (new)
- `crates/api/src/ui/popup/manager.rs` (new)
- `crates/api/src/ui/popup/anchor.rs` (new)
- `crates/api/src/ui/popup/tooltip.rs` (new)
- `crates/api/src/ui/mod.rs` (update)

- [ ] 1.1 Create popup module structure
  - `crates/api/src/ui/popup/mod.rs`
  - Export: `PopupManager`, `PopupAnchor`, `Popup` trait
  
- [ ] 1.2 Define `Popup` trait
  ```rust
  pub trait Popup: Send {
      /// Unique identifier for this popup instance.
      fn id(&self) -> &str;
      
      /// Preferred anchor point for positioning.
      fn anchor(&self) -> PopupAnchor;
      
      /// Minimum and maximum dimensions.
      fn size_hints(&self) -> SizeHints;
      
      /// Handle input event, return whether consumed.
      fn handle_event(&mut self, event: PopupEvent) -> EventResult;
      
      /// Render popup content into the given area.
      fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme);
      
      /// Whether this popup should capture all input (modal).
      fn is_modal(&self) -> bool { false }
  }
  ```

- [ ] 1.3 Define `PopupAnchor` enum
  ```rust
  pub enum PopupAnchor {
      /// Anchor to buffer cursor position.
      Cursor { prefer_above: bool },
      /// Anchor to specific screen position.
      Position { x: u16, y: u16, prefer_above: bool },
      /// Center on screen.
      Center,
  }
  ```

- [ ] 1.4 Implement `PopupManager`
  - Stack of active popups (later popups render on top)
  - Dismiss on Escape or click outside
  - Route events to topmost popup first
  - Calculate final position with collision detection
  
  ```rust
  pub struct PopupManager {
      popups: Vec<Box<dyn Popup>>,
  }
  
  impl PopupManager {
      pub fn show(&mut self, popup: Box<dyn Popup>);
      pub fn dismiss(&mut self, id: &str);
      pub fn dismiss_all(&mut self);
      pub fn has_popups(&self) -> bool;
      
      pub fn handle_event(&mut self, event: PopupEvent) -> bool;
      pub fn render(&self, frame: &mut Frame, cursor_pos: Option<(u16, u16)>, theme: &Theme);
  }
  ```

- [ ] 1.5 Implement position calculation with collision detection
  - File: `crates/api/src/ui/popup/anchor.rs`
  - Flip popup above/below cursor if hitting screen edge
  - Constrain width to available space
  - Handle very long content with scrolling

- [ ] 1.6 Integrate `PopupManager` into `UiManager`
  - Add `popups: PopupManager` field
  - Route events through popup manager before panels
  - Render popups after everything else (on top)

- [ ] 1.7 Create `TooltipPopup` base implementation
  - Simple text/markdown display
  - Auto-sized to content
  - Dismiss on any key or mouse move
  
- [ ] 1.8 Verify: `cargo build --workspace && cargo test --workspace`

**CHECKPOINT 1**: Popup infrastructure exists, can show/dismiss/position popups

---

### Phase 2: Inline Diagnostics

**Objective**: Display LSP diagnostics in gutter, as underlines, and as inline messages.

**Files**:
- `crates/api/src/render/buffer/diagnostics.rs` (new)
- `crates/api/src/render/buffer/context.rs` (update)
- `crates/registry/gutter/src/impls/signs.rs` (update)
- `crates/api/src/editor/mod.rs` (update)

- [ ] 2.1 Create diagnostics rendering module
  - File: `crates/api/src/render/buffer/diagnostics.rs`
  - Convert LSP `Diagnostic` to display spans
  - Map LSP `Range` to buffer char indices
  - Group diagnostics by line for gutter
  
  ```rust
  pub struct DiagnosticDisplay {
      pub line: usize,
      pub start_col: usize,
      pub end_col: usize,
      pub severity: DiagnosticSeverity,
      pub message: String,
      pub source: Option<String>,
  }
  
  pub fn prepare_diagnostics(
      buffer: &Buffer,
      diagnostics: &[lsp_types::Diagnostic],
      encoding: OffsetEncoding,
  ) -> Vec<DiagnosticDisplay>;
  ```

- [ ] 2.2 Add diagnostic underline styles to theme
  - File: `crates/registry/themes/src/colors.rs` or equivalent
  - Add: `diagnostic_error`, `diagnostic_warning`, `diagnostic_info`, `diagnostic_hint`
  - Use underline modifier with color

- [ ] 2.3 Wire diagnostics to gutter signs
  - File: `crates/registry/gutter/src/impls/signs.rs`
  - Currently checks `ctx.annotations.diagnostic_severity`
  - Need to populate this from actual LSP diagnostics
  - File: `crates/api/src/render/buffer/context.rs`
  - In `build_line_annotations()`, set `diagnostic_severity` from prepared diagnostics

- [ ] 2.4 Add underline spans to style overlays
  - File: `crates/api/src/render/buffer/context.rs`
  - Add diagnostic spans alongside syntax highlighting
  - Underline style based on severity

- [ ] 2.5 Add virtual text for first diagnostic per line
  - End-of-line display: ` -- error: message truncated...`
  - Dimmed style, truncate to fit
  - Only show for error/warning severity

- [ ] 2.6 Poll diagnostics on tick
  - File: `crates/api/src/editor/mod.rs`
  - In `tick()`, check for diagnostic updates from `DocumentStateManager`
  - Trigger redraw if diagnostics changed

- [ ] 2.7 Add `]d` / `[d` keybindings for diagnostic navigation
  - Jump to next/prev diagnostic location
  - Wrap around document
  - Show notification with diagnostic message

- [ ] 2.8 Verify: `cargo build --workspace && cargo test --workspace`
- [ ] 2.9 Manual test: Open Rust file with errors, verify gutter/underlines appear

**CHECKPOINT 2**: Diagnostics visible in editor (gutter signs, underlines, inline messages)

---

### Phase 3: Hover Tooltip

**Objective**: Show type information and documentation on demand.

**Files**:
- `crates/api/src/ui/popup/hover.rs` (new)
- `crates/api/src/lsp_ui.rs` (new - bridge between LSP and UI)
- `crates/registry/actions/src/impls/lsp.rs` (new or update)

- [ ] 3.1 Create `HoverPopup` implementation
  - File: `crates/api/src/ui/popup/hover.rs`
  - Implements `Popup` trait
  - Renders markdown content from `Hover` response
  - Auto-dismiss on cursor move or any key
  
  ```rust
  pub struct HoverPopup {
      content: HoverContent,
      anchor: PopupAnchor,
  }
  
  impl HoverPopup {
      pub fn from_hover(hover: Hover, cursor_pos: (u16, u16)) -> Self;
  }
  ```

- [ ] 3.2 Create LSP-UI bridge module
  - File: `crates/api/src/lsp_ui.rs`
  - Coordinates async LSP requests with UI updates
  
  ```rust
  impl Editor {
      pub async fn show_hover(&mut self) {
          let buffer = self.buffer();
          if let Some(hover) = self.lsp.hover(buffer).await.ok().flatten() {
              let cursor_screen_pos = self.cursor_screen_position();
              let popup = HoverPopup::from_hover(hover, cursor_screen_pos);
              self.ui.popups.show(Box::new(popup));
          }
      }
  }
  ```

- [ ] 3.3 Add markdown rendering for hover content
  - Parse `MarkupContent` or `MarkedString` from LSP
  - Convert to styled `Text` for TUI rendering
  - Handle code blocks with syntax highlighting (optional)

- [ ] 3.4 Register `:hover` command and `K` keybinding
  - File: `crates/registry/commands/src/impls/lsp.rs`
  - Trigger `Editor::show_hover()`
  - Normal mode: `K` shows hover at cursor

- [ ] 3.5 Add cursor position tracking for popup anchoring
  - Need to know screen coordinates of buffer cursor
  - File: `crates/api/src/render/buffer/context.rs`
  - Store last rendered cursor position

- [ ] 3.6 Verify: `cargo build --workspace && cargo test --workspace`
- [ ] 3.7 Manual test: Press `K` on Rust identifier, verify hover appears

**CHECKPOINT 3**: Hover tooltip functional with markdown rendering

---

### Phase 4: Completion Menu

**Objective**: Show and filter completion suggestions during insert mode.

**Files**:
- `crates/api/src/ui/popup/completion.rs` (new)
- `crates/api/src/editor/completion.rs` (new)
- `crates/registry/actions/src/impls/insert.rs` (update)

- [ ] 4.1 Create `CompletionPopup` implementation
  - File: `crates/api/src/ui/popup/completion.rs`
  - Scrollable list of completion items
  - Icon by completion kind (function, variable, snippet, etc.)
  - Detail text (type signature) on right
  - Selected item highlighting
  
  ```rust
  pub struct CompletionPopup {
      items: Vec<CompletionItem>,
      filtered: Vec<usize>,  // Indices into items
      selected: usize,
      filter_text: String,
  }
  
  impl CompletionPopup {
      pub fn from_response(response: CompletionResponse) -> Self;
      pub fn filter(&mut self, text: &str);
      pub fn select_next(&mut self);
      pub fn select_prev(&mut self);
      pub fn selected_item(&self) -> Option<&CompletionItem>;
  }
  ```

- [ ] 4.2 Implement completion item icons
  - Map `CompletionItemKind` to Unicode symbols or short text
  - Function: `fn`, Variable: `var`, Snippet: `snip`, etc.
  - Color by kind

- [ ] 4.3 Create completion state machine
  - File: `crates/api/src/editor/completion.rs`
  - States: Inactive, Requesting, Active, Inserting
  - Track trigger character (`.`, `::`, etc.)
  - Handle incremental filtering as user types

- [ ] 4.4 Implement completion triggers
  - Manual: `<C-Space>` in insert mode
  - Automatic: After `.`, `::`, `(` (configurable)
  - Re-trigger on backspace if popup open

- [ ] 4.5 Implement completion acceptance
  - `<Tab>` or `<CR>` accepts selected item
  - Apply `textEdit` or `insertText`
  - Handle `additionalTextEdits` (auto-imports)
  - Handle snippets (basic: just insert text, skip placeholders initially)

- [ ] 4.6 Implement completion dismissal
  - `<Esc>` dismisses
  - Continue typing non-matching chars dismisses
  - Moving cursor dismisses

- [ ] 4.7 Add detail/documentation preview (optional)
  - Show selected item's documentation below list
  - Or in side panel

- [ ] 4.8 Verify: `cargo build --workspace && cargo test --workspace`
- [ ] 4.9 Manual test: Type `.` after struct, verify completions appear and work

**CHECKPOINT 4**: Completion menu functional with filtering and insertion

---

### Phase 5: Signature Help

**Objective**: Show function parameter hints during call expressions.

**Files**:
- `crates/api/src/ui/popup/signature.rs` (new)
- `crates/lsp/src/client/mod.rs` (add signature_help method if missing)

- [ ] 5.1 Add `signature_help()` method to `ClientHandle`
  - File: `crates/lsp/src/client/mod.rs`
  - Similar pattern to hover/completion
  
  ```rust
  pub async fn signature_help(
      &self,
      uri: Url,
      position: Position,
  ) -> Result<Option<SignatureHelp>> {
      self.request::<SignatureHelpRequest>(SignatureHelpParams { ... }).await
  }
  ```

- [ ] 5.2 Add `signature_help()` to `LspManager`
  - File: `crates/api/src/lsp.rs`
  - Delegate to client with position conversion

- [ ] 5.3 Create `SignaturePopup` implementation
  - File: `crates/api/src/ui/popup/signature.rs`
  - Show function signature with active parameter highlighted
  - Cycle through overloads with `<C-n>` / `<C-p>`
  
  ```rust
  pub struct SignaturePopup {
      signatures: Vec<SignatureInformation>,
      active_signature: usize,
      active_parameter: Option<usize>,
  }
  ```

- [ ] 5.4 Implement signature help triggers
  - Automatic: After `(` inside function call
  - Update: On `,` to advance to next parameter
  - Dismiss: After `)` or cursor leaves call

- [ ] 5.5 Render active parameter with emphasis
  - Bold or highlight the current parameter
  - Use `activeParameter` from response
  - Fall back to counting commas

- [ ] 5.6 Verify: `cargo build --workspace && cargo test --workspace`
- [ ] 5.7 Manual test: Type `foo(` for function, verify signature appears

**CHECKPOINT 5**: Signature help shows parameter hints during function calls

---

### Phase 6: Code Actions

**Objective**: Show available quickfixes and refactors with lightbulb gutter indicator.

**Files**:
- `crates/api/src/ui/popup/code_actions.rs` (new)
- `crates/lsp/src/client/mod.rs` (code_action method exists)
- `crates/api/src/lsp.rs` (add code_action wrapper)

- [ ] 6.1 Add `code_action()` method to `LspManager`
  - File: `crates/api/src/lsp.rs`
  - Request code actions for current line/selection
  
  ```rust
  pub async fn code_actions(
      &self,
      buffer: &Buffer,
      range: Option<Range>,
  ) -> Result<Option<Vec<CodeActionOrCommand>>>;
  ```

- [ ] 6.2 Create lightbulb gutter indicator
  - When line has available code actions, show lightbulb icon
  - File: `crates/registry/gutter/src/impls/signs.rs`
  - Add `has_code_actions` to `LineAnnotations`
  - Lightbulb takes priority over other signs

- [ ] 6.3 Create `CodeActionsPopup` implementation
  - File: `crates/api/src/ui/popup/code_actions.rs`
  - List of available actions with descriptions
  - Group by kind (quickfix, refactor, source)
  
  ```rust
  pub struct CodeActionsPopup {
      actions: Vec<CodeActionOrCommand>,
      selected: usize,
  }
  ```

- [ ] 6.4 Implement code action execution
  - On selection, apply `WorkspaceEdit` or execute `Command`
  - Handle document changes across files
  - Show notification on completion

- [ ] 6.5 Add keybindings
  - `<leader>a` or `<C-.>` - show code actions at cursor
  - Auto-show on error line (optional)

- [ ] 6.6 Verify: `cargo build --workspace && cargo test --workspace`
- [ ] 6.7 Manual test: On line with unused import, verify quickfix appears

**CHECKPOINT 6**: Code actions accessible with lightbulb indicator

---

### Phase 7: Diagnostic Panel

**Objective**: Provide persistent list of all diagnostics with navigation.

**Files**:
- `crates/api/src/ui/panels/diagnostics.rs` (new)

- [ ] 7.1 Create `DiagnosticsPanel` implementing `Panel` trait
  - List format: `filename:line:col  severity  message`
  - Grouped by file
  - Scrollable with selection
  
  ```rust
  pub struct DiagnosticsPanel {
      diagnostics: Vec<DiagnosticEntry>,
      selected: usize,
      scroll_offset: usize,
  }
  ```

- [ ] 7.2 Implement navigation
  - `<CR>` jumps to selected diagnostic
  - `]d` / `[d` in panel moves selection
  - Auto-scroll to keep selection visible

- [ ] 7.3 Add filtering options
  - Filter by severity (errors only, warnings+errors, all)
  - Filter by file (current buffer only)

- [ ] 7.4 Wire to dock system
  - Default slot: Bottom
  - Toggle command: `:diagnostics` or `<leader>d`

- [ ] 7.5 Implement live updates
  - Panel refreshes when diagnostics change
  - Preserve selection if possible

- [ ] 7.6 Verify: `cargo build --workspace && cargo test --workspace`

**CHECKPOINT 7**: Diagnostic panel with navigation

---

### Phase 8: Navigation Features

**Objective**: Implement goto definition preview and find references panel.

**Files**:
- `crates/api/src/navigation.rs` (new)
- `crates/api/src/ui/panels/references.rs` (new)

- [ ] 8.1 Enhance goto definition
  - Already have `LspManager::goto_definition()`
  - Add: open file at location
  - Add: preview popup before jumping (optional)
  - Handle multiple definitions (picker)

- [ ] 8.2 Add `:definition` command and `gd` keybinding
  - Jump to definition
  - If multiple, show picker popup

- [ ] 8.3 Create `ReferencesPanel` for find references
  - Similar to diagnostics panel
  - List: `filename:line  context_snippet`
  - Navigate with `<CR>`

- [ ] 8.4 Add `:references` command and `gr` keybinding
  - Open references panel with results
  - If only one reference, jump directly (optional)

- [ ] 8.5 Add peek definition (optional enhancement)
  - Inline preview without leaving current location
  - Floating window with definition context

- [ ] 8.6 Verify: `cargo build --workspace && cargo test --workspace`
- [ ] 8.7 Manual test: `gd` on function call, verify jump works

**CHECKPOINT 8**: Navigation features functional

---

### Phase 9: Inlay Hints (Advanced)

**Objective**: Display type annotations and parameter names as virtual text.

**Files**:
- `crates/api/src/render/buffer/inlay_hints.rs` (new)
- `crates/lsp/src/client/mod.rs` (add inlay_hints method)

- [ ] 9.1 Add `inlay_hints()` method to `ClientHandle`
  - Request inlay hints for visible range
  
  ```rust
  pub async fn inlay_hints(
      &self,
      uri: Url,
      range: Range,
  ) -> Result<Option<Vec<InlayHint>>>;
  ```

- [ ] 9.2 Add `inlay_hints()` to `LspManager`
  - Request for visible buffer range only (performance)
  - Cache hints, re-request on scroll/change

- [ ] 9.3 Implement virtual text rendering
  - Insert hint text inline without affecting cursor/selection
  - Dimmed style to distinguish from actual code
  - Types: after variable (`: Type`)
  - Parameters: before argument (`name:`)

- [ ] 9.4 Add configuration option
  - `inlay-hints-enabled: bool` (default true)
  - Per-kind toggles (type hints, parameter hints)

- [ ] 9.5 Optimize for performance
  - Only request for visible range
  - Debounce requests on rapid scrolling
  - Cache and invalidate on document change

- [ ] 9.6 Verify: `cargo build --workspace && cargo test --workspace`
- [ ] 9.7 Manual test: Open Rust file, verify type hints appear

**CHECKPOINT 9**: Inlay hints render as virtual text

---

## Architecture

### Popup System

```
┌─────────────────────────────────────────────────────────────┐
│ PopupManager                                                 │
├─────────────────────────────────────────────────────────────┤
│ popups: Vec<Box<dyn Popup>>  (stack, last = topmost)        │
│                                                              │
│ show(popup) → push to stack                                  │
│ dismiss(id) → remove by id                                   │
│ dismiss_all() → clear stack                                  │
│                                                              │
│ handle_event(event) → route to topmost, propagate if not    │
│                       consumed, dismiss on Esc/click-out    │
│                                                              │
│ render(frame, cursor_pos, theme) →                          │
│   for each popup:                                           │
│     calculate_position(anchor, size, screen_bounds)         │
│     render(frame, calculated_area, theme)                    │
└─────────────────────────────────────────────────────────────┘
          │
          │ implements
          ▼
┌─────────────────────────────────────────────────────────────┐
│ Popup trait                                                  │
├─────────────────────────────────────────────────────────────┤
│ + id() → &str                                               │
│ + anchor() → PopupAnchor                                    │
│ + size_hints() → SizeHints { min, max, preferred }          │
│ + handle_event(PopupEvent) → EventResult                    │
│ + render(Frame, Rect, Theme)                                │
│ + is_modal() → bool                                         │
└─────────────────────────────────────────────────────────────┘
          △
          │ implemented by
          │
    ┌─────┴─────┬──────────────┬───────────────┬──────────────┐
    │           │              │               │              │
┌───┴───┐  ┌────┴────┐  ┌──────┴──────┐  ┌─────┴─────┐  ┌─────┴─────┐
│Tooltip│  │Completion│  │SignatureHelp│  │CodeActions│  │   Menu    │
│ Popup │  │  Popup   │  │   Popup     │  │  Popup    │  │  Popup    │
└───────┘  └──────────┘  └─────────────┘  └───────────┘  └───────────┘
```

### LSP → UI Data Flow

```
Language Server Process
         │
         │ (JSON-RPC: publishDiagnostics, responses)
         ▼
┌────────────────────────┐
│ xeno-lsp               │
│ ├── MainLoop           │ ← Receives diagnostics, stores in DocumentState
│ ├── ClientHandle       │ ← Sends requests, returns typed responses
│ └── DocumentStateManager│
└───────────┬────────────┘
            │
            ▼
┌────────────────────────┐
│ LspManager             │ ← High-level buffer-centric API
│ (xeno-api/src/lsp.rs)  │
└───────────┬────────────┘
            │
            ├─────────────────────┬─────────────────────┐
            ▼                     ▼                     ▼
┌───────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ Popup System      │  │ Style Overlays   │  │ Panels           │
│ ├── HoverPopup    │  │ ├── Underlines   │  │ ├── Diagnostics  │
│ ├── CompletionPopup│  │ ├── Inlay hints │  │ └── References   │
│ ├── SignaturePopup│  │ └── Virtual text │  │                  │
│ └── CodeActions   │  │                  │  │                  │
└───────────────────┘  └──────────────────┘  └──────────────────┘
            │                     │                     │
            └─────────────────────┴─────────────────────┘
                                  │
                                  ▼
                           ┌──────────┐
                           │ Terminal │
                           │ Render   │
                           └──────────┘
```

### Integration Points

```
┌─────────────────────────────────────────────────────────────┐
│ Editor                                                       │
├─────────────────────────────────────────────────────────────┤
│ lsp: LspManager                 ← LSP client management     │
│ ui: UiManager                   ← Panels + Focus            │
│   └── popups: PopupManager      ← NEW: Popup stack          │
│ style_overlays: StyleOverlays   ← Highlighting + diagnostics│
│                                                              │
│ render(&mut frame) →                                        │
│   1. render_split_buffers()     ← Buffer content + overlays │
│   2. ui.render_panels()         ← Dock panels               │
│   3. ui.popups.render()         ← NEW: Popup overlays       │
│   4. render_notifications()     ← Toast messages            │
└─────────────────────────────────────────────────────────────┘
```

---

## Keybinding Summary

| Binding | Mode | Action |
|---------|------|--------|
| `K` | Normal | Show hover info |
| `gd` | Normal | Goto definition |
| `gr` | Normal | Find references |
| `]d` | Normal | Next diagnostic |
| `[d` | Normal | Previous diagnostic |
| `<leader>a` | Normal | Code actions |
| `<C-Space>` | Insert | Trigger completion |
| `<Tab>` | Insert (completion) | Accept completion |
| `<C-n>` | Insert (completion) | Next completion |
| `<C-p>` | Insert (completion) | Previous completion |
| `<Esc>` | Any popup | Dismiss popup |

---

## Anti-Patterns

1. **Blocking UI on LSP requests**: Never block the render loop. All LSP requests must be async with loading indicators if slow.

2. **Popup position hardcoding**: Always calculate position relative to cursor/anchor with collision detection. Never assume fixed screen size.

3. **Ignoring encoding**: Always use `OffsetEncoding` from server capabilities. UTF-16 vs UTF-8 positions cause off-by-one errors.

4. **Unbounded popup content**: Always constrain popup dimensions. Truncate or scroll long content. Never let popup exceed screen.

5. **Polling too frequently**: Debounce completion requests. Don't request on every keystroke - wait for pause or trigger char.

6. **Forgetting cleanup**: Dismiss popups when buffer changes, file closes, or focus moves. Stale popups are confusing.

7. **Tight LSP coupling**: UI components should not depend on specific LSP types. Convert to display types at boundary.

---

## Success Criteria

- [ ] Popup infrastructure exists with proper positioning and event handling
- [ ] Diagnostics visible: gutter signs, underlines, and inline messages
- [ ] Hover tooltip shows documentation on `K`
- [ ] Completion menu works with filtering, selection, and insertion
- [ ] Signature help shows during function calls
- [ ] Code actions accessible with lightbulb indicator
- [ ] Diagnostic panel lists all errors with navigation
- [ ] `gd` and `gr` work for navigation
- [ ] All features work with rust-analyzer (primary test target)
- [ ] No regressions in existing functionality
- [ ] All tests passing
- [ ] No clippy warnings

---

## Files Summary

| File | Type | Phase |
|------|------|-------|
| `crates/api/src/ui/popup/mod.rs` | New | 1 |
| `crates/api/src/ui/popup/manager.rs` | New | 1 |
| `crates/api/src/ui/popup/anchor.rs` | New | 1 |
| `crates/api/src/ui/popup/tooltip.rs` | New | 1, 3 |
| `crates/api/src/ui/popup/completion.rs` | New | 4 |
| `crates/api/src/ui/popup/signature.rs` | New | 5 |
| `crates/api/src/ui/popup/code_actions.rs` | New | 6 |
| `crates/api/src/render/buffer/diagnostics.rs` | New | 2 |
| `crates/api/src/ui/panels/diagnostics.rs` | New | 7 |
| `crates/api/src/ui/panels/references.rs` | New | 8 |
| `crates/api/src/render/buffer/inlay_hints.rs` | New | 9 |
| `crates/api/src/lsp_ui.rs` | New | 3 |
| `crates/api/src/navigation.rs` | New | 8 |
| `crates/api/src/editor/completion.rs` | New | 4 |
| `crates/api/src/ui/mod.rs` | Update | 1 |
| `crates/api/src/ui/manager.rs` | Update | 1 |
| `crates/api/src/render/buffer/context.rs` | Update | 2 |
| `crates/registry/gutter/src/impls/signs.rs` | Update | 2, 6 |
| `crates/api/src/lsp.rs` | Update | 5, 6, 9 |
| `crates/lsp/src/client/mod.rs` | Update | 5, 9 |
| `crates/registry/actions/src/impls/lsp.rs` | New | 3, 8 |

---

## Testing Strategy

### Unit Tests

- Popup positioning with various screen sizes and cursor positions
- Completion filtering logic
- Diagnostic range → buffer position conversion
- Inlay hint insertion point calculation

### Integration Tests

- Mock LSP responses → verify UI state changes
- Event handling → popup dismiss behavior
- Keybinding → action routing

### Manual Testing

Required language servers for testing:
- **rust-analyzer** (primary): All features
- **typescript-language-server**: Verify cross-language compatibility
- **gopls**: Additional coverage

Test scenarios:
1. Hover on type → verify docs appear
2. Complete struct field → verify insertion
3. Error in code → verify gutter, underline, and message
4. Multiple definitions → verify picker appears
5. Large file → verify no performance regression
