# Evildoer: Viewport-Aware Motions Implementation

## Model Directive

Design and implement viewport-aware motions (`screen_top`, `screen_middle`, `screen_bottom`) that require viewport context not available in the current motion handler signature. This completes the typed handle migration by eliminating the deferred screen motion stubs.

**Go fully through with implementation** - don't stop at sketches or partial work.

______________________________________________________________________

## Context

The current motion handler signature is:
```rust
pub type MotionHandler = fn(RopeSlice, Range, usize, bool) -> Range;
//                          text      range  count extend
```

This only has access to document text, not viewport state. Screen-relative motions (H/M/L in vim) need to know:
- First visible line
- Last visible line  
- Viewport height

Currently these actions return errors:
```rust
// crates/registry/actions/src/impls/motions.rs
action!(move_top_screen, { ... }, |_ctx| {
    ActionResult::Error("screen_top motion requires viewport context".to_string())
});
```

______________________________________________________________________

## Design Options

### Option A: Extend MotionHandler Signature

Add viewport info to all motion handlers:
```rust
pub struct MotionContext<'a> {
    pub text: RopeSlice<'a>,
    pub viewport: Option<ViewportInfo>,
}

pub struct ViewportInfo {
    pub first_visible_line: usize,
    pub last_visible_line: usize,
    pub height: usize,
}

pub type MotionHandler = fn(MotionContext, Range, usize, bool) -> Range;
```

**Pros:** Unified API, all motions can use viewport if needed
**Cons:** Breaking change to all motion handlers, most don't need viewport

### Option B: Separate Viewport Motion Type

Create a distinct motion type for viewport-aware motions:
```rust
pub type ViewportMotionHandler = fn(RopeSlice, Range, ViewportInfo, usize, bool) -> Range;

pub enum MotionKind {
    Document(MotionHandler),
    Viewport(ViewportMotionHandler),
}
```

**Pros:** Non-breaking, explicit about requirements
**Cons:** Two motion types, more complex dispatch

### Option C: ActionContext Extension

Screen motions aren't really "motions" in the document sense - they're actions that need editor state. Handle them as special ActionResult variants:
```rust
pub enum ActionResult {
    // ... existing variants
    ScreenMotion(ScreenPosition),
}

pub enum ScreenPosition {
    Top,
    Middle, 
    Bottom,
}
```

The result handler has access to Editor and can compute the actual position.

**Pros:** Clean separation, no motion system changes
**Cons:** Screen "motions" aren't composable with other motions

### Option D: Capability-Based Motion Context

Extend ActionContext with optional viewport access:
```rust
impl ActionContext {
    pub fn viewport(&self) -> Option<&ViewportInfo> { ... }
}
```

Screen motion actions check for viewport and compute position inline.

**Pros:** Minimal changes, explicit capability
**Cons:** Screen motions live in actions, not motion registry

______________________________________________________________________

## Recommended Approach

**Option C (ActionResult::ScreenMotion)** is cleanest because:
1. Screen motions fundamentally need editor state, not just document text
2. They're not composable with text motions anyway (you don't "screen_top then word")
3. Result handlers already have Editor access
4. No changes to motion registry infrastructure

______________________________________________________________________

## Implementation Plan

### Phase 1: Add ScreenMotion ActionResult Variant

**Files:** `crates/registry/actions/src/result.rs`

```rust
/// Screen-relative cursor position.
#[derive(Debug, Clone, Copy)]
pub enum ScreenPosition {
    /// First visible line (vim H)
    Top,
    /// Middle visible line (vim M)
    Middle,
    /// Last visible line (vim L)
    Bottom,
}

pub enum ActionResult {
    // ... existing
    /// Move cursor to screen-relative position.
    ScreenMotion(ScreenPosition),
}
```

### Phase 2: Update Screen Motion Actions

**Files:** `crates/registry/actions/src/impls/motions.rs`

```rust
action!(move_top_screen, { description: "Move to top of screen", bindings: r#"normal "H""# },
    |_ctx| ActionResult::ScreenMotion(ScreenPosition::Top));

action!(move_middle_screen, { description: "Move to middle of screen", bindings: r#"normal "M""# },
    |_ctx| ActionResult::ScreenMotion(ScreenPosition::Middle));

action!(move_bottom_screen, { description: "Move to bottom of screen" },
    |_ctx| ActionResult::ScreenMotion(ScreenPosition::Bottom));
```

### Phase 3: Add Result Handler

**Files:** `crates/core/src/editor_ctx/result_handlers/` or similar

```rust
result_handler!(
    RESULT_SCREEN_MOTION_HANDLERS,
    SCREEN_MOTION_HANDLER,
    "screen_motion",
    |result, ctx, _extend| {
        let ActionResult::ScreenMotion(pos) = result else {
            return HandleOutcome::NotHandled;
        };
        
        // Need viewport access - check what capabilities exist
        // Compute target line based on pos and viewport
        // Move cursor to that line
        
        HandleOutcome::Handled
    }
);
```

### Phase 4: Implement Viewport Logic

The result handler needs to:
1. Get current viewport bounds (first/last visible line)
2. Compute target line based on ScreenPosition
3. Move cursor to start of that line
4. Handle extend mode (create selection from current to target)

______________________________________________________________________

## Key Questions to Resolve

1. **Where does viewport info come from?**
   - Is it in EditorContext capabilities?
   - Does Buffer track viewport?
   - Need to trace through rendering code

2. **How does extend mode work for screen motions?**
   - vim: `vH` selects from cursor to top of screen
   - Need to preserve anchor, move head

3. **What about count?**
   - vim: `3H` = 3rd line from top
   - Should ScreenPosition include offset?

______________________________________________________________________

## Files to Review

```
crates/registry/actions/src/result.rs           # ActionResult enum
crates/registry/actions/src/impls/motions.rs    # Screen motion stubs
crates/core/src/editor_ctx/result_handlers/     # Existing handlers
crates/api/src/editor/mod.rs                    # Editor state
crates/api/src/buffer/mod.rs                    # Buffer/viewport
crates/api/src/render/                          # Viewport calculation
```

______________________________________________________________________

## Success Criteria

- [ ] `ActionResult::ScreenMotion` variant added with derive macros
- [ ] Screen motion actions return typed variant (not error)
- [ ] Result handler computes correct target line
- [ ] H/M/L keybindings work correctly
- [ ] Extend mode works (vH, vM, vL)
- [ ] Count works (3H = 3rd line from top)
- [ ] `cargo test --workspace` passes
- [ ] No more "requires viewport context" errors
