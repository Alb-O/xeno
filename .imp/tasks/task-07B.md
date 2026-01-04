# Task 07B: Per-Context Gutter Customization

## Model Directive

This document specifies a design refinement for the gutter registry system (task-07A). The goal is to enable **trivial per-context gutter customization** without coupling Buffer to rendering concerns.

The current implementation hardcodes `enabled_gutters()` in `GutterLayout::from_registry()`. We need a clean pattern that allows different rendering contexts (palette, terminal, picker, etc.) to specify their own gutter behavior using the existing registry primitives.

---

## Problem Statement

After task-07A, we have:
- `gutter!` macro for registering gutter columns
- `GutterLayout` for rendering gutters
- `Buffer::gutter_width()` delegates to registry

**The gap**: No clean way to say "render this buffer with `>` prompts instead of line numbers" without:
1. Adding fields to Buffer (couples Buffer to rendering)
2. Modifying the render function signature (breaks existing call sites)
3. Creating one-off special cases in rendering code

**Desired usage** (conceptual):
```rust
// In palette rendering - should be this simple:
let gutter = GutterLayout::prompt('>');
// or
let gutter = GutterLayout::from_names(&["prompt"]);
// or
let gutter = GutterLayout::custom(|ctx| ...);
```

---

## Design Constraints

<mandatory_execution_requirements>

1. The solution must NOT add gutter-specific fields to `Buffer`
2. The solution must work with the existing `gutter!` macro pattern
3. Custom gutters must be as easy to define as registry gutters
4. The `render_buffer()` signature should remain stable or change minimally
5. All existing tests must pass without snapshot updates

</mandatory_execution_requirements>

<verbosity_and_scope_constraints>

- Prefer extending `GutterLayout` over creating parallel abstractions
- Avoid trait-based extensibility unless clearly necessary
- Keep the happy path (default line numbers) zero-cost
- No heap allocation for simple cases (prompt char, hidden gutter)

</verbosity_and_scope_constraints>

---

## Design Options to Evaluate

### Option A: Gutter Selector at Render Site

Add a `GutterSelector` that can be passed to rendering:

```rust
pub enum GutterSelector {
    /// Use enabled gutters from registry (default behavior)
    Registry,
    /// Use specific gutters by name
    Named(&'static [&'static str]),
    /// Hide gutter entirely  
    Hidden,
    /// Single prompt character
    Prompt(char),
    /// Custom render function
    Custom { width: u16, render: fn(&GutterLineContext) -> Option<GutterCell> },
}

impl GutterLayout {
    pub fn from_selector(selector: GutterSelector, total_lines: usize, viewport_width: u16) -> Self
}
```

**Pros**: Explicit, no magic, covers all cases
**Cons**: Need to thread selector through render_buffer() or get it from somewhere

### Option B: Gutter Config on Window (not Buffer)

Windows already own the rendering context. Add gutter config there:

```rust
pub struct FloatingWindow {
    pub buffer: BufferId,
    pub rect: Rect,
    pub style: FloatingStyle,
    pub gutter: GutterSelector,  // NEW
    // ...
}
```

**Pros**: Separates rendering concern from Buffer, Window already owns style
**Cons**: Need to plumb Window info into render_buffer()

### Option C: Named Gutter Presets in Registry

Register named presets that bundle gutter configurations:

```rust
gutter_preset!("default", &["line_numbers"]);
gutter_preset!("prompt", &["prompt"]);
gutter_preset!("minimal", &[]);

// Usage:
GutterLayout::from_preset("prompt", total_lines, viewport_width)
```

**Pros**: Declarative, extensible via registry
**Cons**: Indirection, doesn't solve custom render functions

### Option D: Render Context Carries Gutter Config

Create a richer render context that includes gutter selection:

```rust
pub struct BufferRenderConfig {
    pub gutter: GutterSelector,
    pub show_cursor: bool,
    // other render-time options
}

fn render_buffer(&self, buffer: &Buffer, area: Rect, config: BufferRenderConfig) -> RenderResult
```

**Pros**: Clean separation, extensible for other render options
**Cons**: Changes render_buffer() signature

---

## Recommended Approach

Evaluate options by:
1. Reading current render_buffer() call sites
2. Understanding how palette/floating windows are rendered
3. Choosing the option with minimal churn and maximum clarity

The evaluation should answer:
- Where does gutter selection naturally belong? (Buffer? Window? Render call?)
- What's the simplest change that enables `GutterLayout::prompt('>')` for palette?
- Can we avoid changing render_buffer() signature?

---

## Implementation Roadmap

### Phase 1: Design Validation

Objective: Confirm the chosen design works with existing architecture

Tasks:
- 1.1 Read render call sites: `crates/api/src/render/document/mod.rs`, find where `render_buffer` is called
- 1.2 Read floating window rendering: understand how palette buffer is rendered
- 1.3 Decide: which option (A/B/C/D) fits best
- 1.4 Write decision rationale in this file

### Phase 2: Extend GutterLayout

Objective: Add constructors for custom/prompt/hidden gutters

Tasks:
- 2.1 Add `GutterLayout::hidden()` -> zero-width, no columns
- 2.2 Add `GutterLayout::prompt(char)` -> single char column
- 2.3 Add `GutterLayout::custom(width, fn)` -> arbitrary render function
- 2.4 Verify: `cargo check -p xeno-api`

### Phase 3: Wire Up Palette

Objective: Palette uses `>` prompt gutter

Tasks:
- 3.1 Implement chosen wiring approach (Window field, render param, etc.)
- 3.2 Update palette creation to specify prompt gutter
- 3.3 Verify: `cargo test --workspace`

### Phase 4: Cleanup

Objective: Remove any scaffolding, update docs

Tasks:
- 4.1 Remove unused code paths if any
- 4.2 Update AGENTS.md if API changed
- 4.3 Final test run

---

## Success Criteria

1. Palette shows `>` instead of line numbers
2. No `GutterMode` or similar field on Buffer
3. Adding a new custom gutter context requires ~5 lines of code
4. All tests pass, no snapshot changes for existing behavior

---

## Current Code References

| File | Purpose |
|------|---------|
| `crates/api/src/render/buffer/gutter.rs` | GutterLayout implementation |
| `crates/api/src/render/buffer/context.rs` | render_buffer() function |
| `crates/api/src/render/document/mod.rs` | Document rendering, calls render_buffer |
| `crates/api/src/editor/palette.rs` | Palette creation |
| `crates/api/src/window/mod.rs` | Window types |
