# Plan: Viewport-bounded locals (Neovim-style deferred approach)

Status: archived proposal (not implemented on `main`)  
Last verified: 2026-02-10

## Context

The previous plan (skip locals for L-tier files via `build_locals: bool`) showed no measurable performance improvement for large files like `miniaudio.h`. Investigation revealed that Neovim doesn't implement `@local.reference` resolution at all — its highlighting is pure pattern matching.

Rather than a binary on/off flag, this plan implements **viewport-bounded locals**: the locals query runs only over `0..viewport_end_byte` instead of the full file, and is deferred to the render thread rather than running inside the background parse task. This targets all tiers (not just L), reduces background parse cost, and keeps locals accuracy for visible code.

**Graceful degradation is unchanged:** when `Locals` is empty or covers less than the viewport, `lookup_reference` returns `None` for uncovered references. The highlighter falls back to generic highlights (e.g. `(identifier) @variable`). The visual difference is minimal.

### Current state (reality check)

- `build_locals_for_layer` exists as a standalone helper in `tmp/tree-house/highlighter/src/locals.rs`.
- `Syntax::new_without_locals()` exists, but `Syntax::build_locals()` still runs full-range locals (no range parameter yet).
- `Syntax::new()` still delegates to `new_without_locals + build_locals`.
- The full viewport-first syntax pipeline moved toward sealed-window parsing and repair (`TaskKind::ViewportParse`) rather than this locals-specific plan.

The sections below are kept as a proposal reference and are not a description
of current behavior.

## Changes

### Part 1: tree-house — range-parameterized locals

**File: `tmp/tree-house/highlighter/src/locals.rs`**

Add `range: Range<u32>` parameter to `build_locals_for_layer`:

```rust
pub(crate) fn build_locals_for_layer(
    layer_data: &mut LayerData,
    source: RopeSlice<'_>,
    loader: &impl LanguageLoader,
    range: Range<u32>,           // NEW — was hardcoded 0..u32::MAX
) {
    // ...
    let mut cursor = InactiveQueryCursor::new(range, TREE_SITTER_MATCH_LIMIT)
        .execute_query(&injection_query.local_query, &root, source);
    // ... rest unchanged
}
```

**File: `tmp/tree-house/highlighter/src/lib.rs`**

Add `range` parameter to `build_locals`:

```rust
pub fn build_locals(&mut self, source: RopeSlice, loader: &impl LanguageLoader, range: Range<u32>) {
    for (_idx, layer_data) in &mut self.layers {
        locals::build_locals_for_layer(layer_data, source, loader, range.clone());
    }
}
```

Update `Syntax::new()` to pass full range (backward compat for tree-house tests):

```rust
pub fn new(...) -> Result<Self, Error> {
    let mut syntax = Self::new_without_locals(source, language, timeout, loader)?;
    syntax.build_locals(source, loader, 0..u32::MAX);
    Ok(syntax)
}
```

### Part 2: Xeno language crate — deferred locals with coverage tracking

**File: `crates/language/src/syntax/mod.rs`**

Remove `build_locals: bool` from `SyntaxOptions` (revert to just `parse_timeout` + `injections`):

```rust
pub struct SyntaxOptions {
    pub parse_timeout: Duration,
    pub injections: InjectionPolicy,
}
```

Add `locals_byte_end` tracking to `Syntax`:

```rust
pub struct Syntax {
    inner: tree_house::Syntax,
    opts: SyntaxOptions,
    locals_byte_end: u32,  // NEW — coverage boundary, 0 = no locals built
}
```

`Syntax::new()` never builds locals — always deferred:

```rust
pub fn new(source, language, loader, opts) -> Result<Self, SyntaxError> {
    let loader = loader.with_injections(matches!(opts.injections, InjectionPolicy::Eager));
    let inner = tree_house::Syntax::new_without_locals(source, language, opts.parse_timeout, &loader)?;
    Ok(Self { inner, opts, locals_byte_end: 0 })
}
```

`update()` and `update_from_changeset()` reset `locals_byte_end` (tree changed, old locals invalid):

```rust
pub fn update(&mut self, source, edits, loader, opts) -> Result<(), SyntaxError> {
    // ... existing update logic (WITHOUT build_locals call) ...
    self.locals_byte_end = 0;
    Ok(())
}
```

New `ensure_locals` method with geometric growth:

```rust
/// Rebuilds the locals scope tree if current coverage is insufficient.
///
/// Returns `true` if locals were rebuilt (caller should bump syntax version).
/// Uses geometric growth to amortize scroll cost: each rebuild covers at
/// least 2x the previous range, capped at file size.
pub fn ensure_locals(
    &mut self,
    source: RopeSlice,
    loader: &LanguageLoader,
    viewport_end_byte: u32,
) -> bool {
    if self.locals_byte_end >= viewport_end_byte {
        return false;
    }
    let new_end = viewport_end_byte
        .max(self.locals_byte_end.saturating_mul(2))
        .min(source.len_bytes() as u32);
    let loader = loader.with_injections(matches!(self.opts.injections, InjectionPolicy::Eager));
    self.inner.build_locals(source, &loader, 0..new_end);
    self.locals_byte_end = new_end;
    true
}
```

### Part 3: Xeno syntax_manager — remove `build_locals` flag, add `ensure_locals`

**Files: `crates/editor/src/syntax_manager/policy.rs`, `crates/editor/src/syntax_manager/types.rs`, `crates/editor/src/syntax_manager/ensure.rs`**

Remove `build_locals: bool` from `TierCfg`.

Remove `build_locals: bool` from `OptKey`.

Remove `build_locals` from all `SyntaxOptions` construction sites.

Add `ensure_locals` method:

```rust
/// Rebuilds viewport-bounded locals for a document if needed.
///
/// Returns `true` if locals were rebuilt and `syntax_version` was bumped
/// (triggering tile cache invalidation).
pub fn ensure_locals(
    &mut self,
    doc_id: DocumentId,
    content: &Rope,
    loader: &LanguageLoader,
    viewport_end_byte: u32,
) -> bool {
    let Some(entry) = self.entries.get_mut(&doc_id) else { return false };
    let Some(syntax) = entry.slot.current.as_mut() else { return false };
    if syntax.ensure_locals(content.slice(..), loader, viewport_end_byte) {
        entry.slot.version = entry.slot.version.wrapping_add(1);
        true
    } else {
        false
    }
}
```

### Part 4: Editor lifecycle — wire viewport info

**File: `crates/editor/src/impls/lifecycle/ops.rs`**

After the existing `ensure_syntax` loop (line 109), add a second pass for visible documents that builds viewport-bounded locals:

```rust
// --- Viewport-bounded locals pass ---
// Collect max viewport_end_byte per visible document across all views.
let mut doc_viewport_end: HashMap<DocumentId, (u32, Rope)> = HashMap::new();

for buffer in self.state.core.buffers.buffers() {
    if !visible_ids.contains(&buffer.id) {
        continue;
    }
    let doc_id = buffer.document_id();
    let (content, viewport_end_byte) = buffer.with_doc(|doc| {
        let content = doc.content().clone();
        let total_lines = content.len_lines();
        let end_line = (buffer.scroll_line + buffer.last_viewport_height).min(total_lines);
        let end_byte = content.line_to_byte(end_line) as u32;
        (content, end_byte)
    });

    doc_viewport_end
        .entry(doc_id)
        .and_modify(|(existing_end, _)| {
            *existing_end = (*existing_end).max(viewport_end_byte);
        })
        .or_insert((viewport_end_byte, content));
}

for (doc_id, (viewport_end_byte, content)) in &doc_viewport_end {
    if self.state.syntax_manager.ensure_locals(*doc_id, content, &loader, *viewport_end_byte) {
        self.state.effects.request_redraw();
    }
}
```

Note: `last_viewport_height` is 0 on the first render frame (set during rendering at `render/buffer/viewport/ops.rs:33`). This means locals aren't built on frame 1, but the tree isn't available yet either (background parse). On frame 2+, both the tree and viewport height are available.

## Proposed files

| File | Change |
|------|--------|
| `tmp/tree-house/highlighter/src/locals.rs` | Add `range: Range<u32>` param to `build_locals_for_layer` |
| `tmp/tree-house/highlighter/src/lib.rs` | Add `range` param to `build_locals`, update `Syntax::new()` |
| `crates/language/src/syntax/mod.rs` | Remove `build_locals` from `SyntaxOptions`, add `locals_byte_end` + `ensure_locals` to `Syntax` |
| `crates/editor/src/syntax_manager/{policy.rs,types.rs,ensure.rs}` | Remove `build_locals` from scheduler option keys and add `SyntaxManager::ensure_locals` |
| `crates/editor/src/impls/lifecycle/ops.rs` | Add viewport-bounded locals pass after `ensure_syntax` loop |

## Verification (if revived)

1. `cd tmp/tree-house && cargo test -p tree-house` — all fixture tests must pass (`Syntax::new()` still builds full-range locals).

2. `cargo test --workspace` — no regressions in Xeno tests.

3. Open a small Rust file — verify parameter locals still work (consistent highlighting at definition and usage).

4. Open `tmp/miniaudio.h` — verify highlighting works, no crashes, faster background parse (locals no longer run in the background task).

5. Scroll through a large file — verify locals extend on scroll (geometric growth means infrequent rebuilds).
