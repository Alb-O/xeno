# Task 07A: Gutter Registry System

## Summary

Replace hardcoded line number rendering with a composable, closure-based gutter registry. Enables arbitrary text/math for line numbers and supports multiple gutter columns (signs, diagnostics, git status).

## Design Decisions

1. **Width caching**: Recompute each render (simple, correct). Cache only if profiling shows it's a bottleneck.

2. **Column ordering**: Priority-based (lower = further left). No positional enum needed - priority is more flexible for extensions inserting between builtin columns.

3. **Hybrid line numbers**: Built-in as `hybrid_line_numbers` - absolute on cursor line, relative elsewhere. Common enough to warrant first-class support.

4. **Annotations API**: Start with concrete `GutterAnnotations` struct with optional fields. Trait-based extension adds complexity without clear benefit yet. Extensions can add fields via feature flags.

---

## Phase 1: Create Registry Crate

### Files to Create

| Path | Purpose |
|------|---------|
| `crates/registry/gutter/Cargo.toml` | Crate manifest |
| `crates/registry/gutter/src/lib.rs` | Types, distributed slice, helpers |
| `crates/registry/gutter/src/macros.rs` | `gutter!` macro |

### Core Types

```rust
// crates/registry/gutter/src/lib.rs

use linkme::distributed_slice;
use ropey::RopeSlice;
use std::path::Path;
use xeno_tui::style::Style;

pub use xeno_registry_core::{RegistryMetadata, RegistrySource, impl_registry_metadata};

mod macros;
mod impls;

/// Context passed to each gutter render closure (per-line).
pub struct GutterLineContext<'a> {
    /// 0-indexed line number in document.
    pub line_idx: usize,
    /// Total lines in the document.
    pub total_lines: usize,
    /// Current cursor line (0-indexed) - enables relative line numbers.
    pub cursor_line: usize,
    /// Whether this line is the cursor line.
    pub is_cursor_line: bool,
    /// Whether this is a wrapped continuation (not first segment of line).
    pub is_continuation: bool,
    /// Line text (rope slice for efficient access).
    pub line_text: RopeSlice<'a>,
    /// File path if available.
    pub path: Option<&'a Path>,
    /// Per-line annotation data (diagnostics, git, etc.).
    pub annotations: &'a GutterAnnotations,
}

/// Context for width calculation (per-document, not per-line).
pub struct GutterWidthContext {
    /// Total lines in document.
    pub total_lines: usize,
    /// Maximum viewport width (for constraints).
    pub viewport_width: u16,
}

/// What a gutter column renders for a single line.
#[derive(Debug, Clone)]
pub struct GutterCell {
    /// Text content (right-aligned within column width by renderer).
    pub text: String,
    /// Style hint.
    pub style: GutterStyle,
}

/// Style hints for gutter cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GutterStyle {
    /// Normal line number color.
    #[default]
    Normal,
    /// Dimmed (continuations, empty lines).
    Dim,
    /// Highlighted (cursor line).
    Cursor,
}

/// Width calculation strategy.
#[derive(Debug, Clone, Copy)]
pub enum GutterWidth {
    /// Fixed width in characters.
    Fixed(u16),
    /// Dynamic width computed from document state.
    Dynamic(fn(&GutterWidthContext) -> u16),
}

/// Per-line annotation data for gutter columns.
/// Extended by LSP/git extensions via optional fields.
#[derive(Debug, Clone, Default)]
pub struct GutterAnnotations {
    /// Diagnostic severity (0=none, 1=hint, 2=info, 3=warn, 4=error).
    pub diagnostic_severity: u8,
    /// Git hunk type: None, Added, Modified, Removed.
    pub git_status: Option<GitHunkStatus>,
    /// Custom sign character (breakpoint, bookmark, etc.).
    pub sign: Option<char>,
}

/// Git hunk status for gutter display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHunkStatus {
    Added,
    Modified,
    Removed,
}

/// Definition of a gutter column.
pub struct GutterDef {
    /// Unique identifier (e.g., "xeno_registry_gutter::line_numbers").
    pub id: &'static str,
    /// Column name (e.g., "line_numbers").
    pub name: &'static str,
    /// Short description.
    pub description: &'static str,
    /// Priority: lower = renders further left.
    pub priority: i16,
    /// Whether enabled by default.
    pub default_enabled: bool,
    /// Width strategy.
    pub width: GutterWidth,
    /// Render function - called per visible line.
    pub render: fn(&GutterLineContext) -> Option<GutterCell>,
    /// Origin of the column.
    pub source: RegistrySource,
}

impl core::fmt::Debug for GutterDef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GutterDef")
            .field("name", &self.name)
            .field("priority", &self.priority)
            .field("default_enabled", &self.default_enabled)
            .finish()
    }
}

/// Registry of all gutter column definitions.
#[distributed_slice]
pub static GUTTERS: [GutterDef];

/// Returns enabled gutters sorted by priority (left to right).
pub fn enabled_gutters() -> impl Iterator<Item = &'static GutterDef> {
    let mut gutters: Vec<_> = GUTTERS
        .iter()
        .filter(|g| g.default_enabled)
        .collect();
    gutters.sort_by_key(|g| g.priority);
    gutters.into_iter()
}

/// Finds a gutter column by name.
pub fn find(name: &str) -> Option<&'static GutterDef> {
    GUTTERS.iter().find(|g| g.name == name)
}

/// Returns all registered gutter columns.
pub fn all() -> impl Iterator<Item = &'static GutterDef> {
    GUTTERS.iter()
}

/// Computes total gutter width from enabled columns.
pub fn total_width(ctx: &GutterWidthContext) -> u16 {
    enabled_gutters()
        .map(|g| match g.width {
            GutterWidth::Fixed(w) => w,
            GutterWidth::Dynamic(f) => f(ctx),
        })
        .sum::<u16>()
        + 1 // trailing separator space
}

impl_registry_metadata!(GutterDef);
```

### Macro

```rust
// crates/registry/gutter/src/macros.rs

/// Registers a gutter column in the [`GUTTERS`] slice.
///
/// # Examples
///
/// ```ignore
/// gutter!(line_numbers, {
///     description: "Absolute line numbers",
///     priority: 0,
///     width: Dynamic(|ctx| (ctx.total_lines.max(1).ilog10() as u16 + 2).max(4)),
/// }, |ctx| {
///     Some(GutterCell {
///         text: format!("{}", ctx.line_idx + 1),
///         style: GutterStyle::Normal,
///     })
/// });
/// ```
#[macro_export]
macro_rules! gutter {
    ($name:ident, {
        description: $desc:expr,
        priority: $priority:expr,
        width: $width:expr
        $(, enabled: $enabled:expr)?
    }, $render:expr) => {
        paste::paste! {
            #[allow(non_upper_case_globals)]
            #[linkme::distributed_slice($crate::GUTTERS)]
            static [<GUTTER_ $name:upper>]: $crate::GutterDef = $crate::GutterDef {
                id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
                name: stringify!($name),
                description: $desc,
                priority: $priority,
                default_enabled: $crate::__gutter_enabled!($($enabled)?),
                width: $crate::GutterWidth::$width,
                render: $render,
                source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
            };
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __gutter_enabled {
    () => { true };
    ($val:expr) => { $val };
}
```

---

## Phase 2: Built-in Gutter Implementations

### Files to Create

| Path | Purpose |
|------|---------|
| `crates/registry/gutter/src/impls/mod.rs` | Module exports |
| `crates/registry/gutter/src/impls/line_numbers.rs` | Absolute line numbers |
| `crates/registry/gutter/src/impls/relative.rs` | Relative line numbers |
| `crates/registry/gutter/src/impls/hybrid.rs` | Hybrid (absolute on cursor, relative elsewhere) |
| `crates/registry/gutter/src/impls/signs.rs` | Sign column placeholder |

### Implementations

**Absolute line numbers** (default enabled, priority 0):
```rust
gutter!(line_numbers, {
    description: "Absolute line numbers",
    priority: 0,
    width: Dynamic(|ctx| (ctx.total_lines.max(1).ilog10() as u16 + 2).max(4)),
}, |ctx| {
    if ctx.is_continuation {
        Some(GutterCell { text: "\u{2506}".into(), style: GutterStyle::Dim })
    } else {
        Some(GutterCell {
            text: format!("{}", ctx.line_idx + 1),
            style: if ctx.is_cursor_line { GutterStyle::Cursor } else { GutterStyle::Normal },
        })
    }
});
```

**Relative line numbers** (disabled by default, priority 0):
```rust
gutter!(relative_line_numbers, {
    description: "Relative line numbers from cursor",
    priority: 0,
    width: Dynamic(|ctx| (ctx.total_lines.max(1).ilog10() as u16 + 2).max(4)),
    enabled: false,
}, |ctx| {
    if ctx.is_continuation {
        Some(GutterCell { text: "\u{2506}".into(), style: GutterStyle::Dim })
    } else {
        let distance = ctx.line_idx.abs_diff(ctx.cursor_line);
        Some(GutterCell {
            text: format!("{}", distance),
            style: if ctx.is_cursor_line { GutterStyle::Cursor } else { GutterStyle::Normal },
        })
    }
});
```

**Hybrid line numbers** (disabled by default, priority 0):
```rust
gutter!(hybrid_line_numbers, {
    description: "Absolute on cursor line, relative elsewhere",
    priority: 0,
    width: Dynamic(|ctx| (ctx.total_lines.max(1).ilog10() as u16 + 2).max(4)),
    enabled: false,
}, |ctx| {
    if ctx.is_continuation {
        Some(GutterCell { text: "\u{2506}".into(), style: GutterStyle::Dim })
    } else {
        let display = if ctx.is_cursor_line {
            ctx.line_idx + 1
        } else {
            ctx.line_idx.abs_diff(ctx.cursor_line)
        };
        Some(GutterCell {
            text: format!("{}", display),
            style: if ctx.is_cursor_line { GutterStyle::Cursor } else { GutterStyle::Normal },
        })
    }
});
```

**Signs column** (enabled by default, priority -10 = left of line numbers):
```rust
gutter!(signs, {
    description: "Sign column for diagnostics and markers",
    priority: -10,
    width: Fixed(2),
}, |ctx| {
    if let Some(sign) = ctx.annotations.sign {
        return Some(GutterCell { text: sign.to_string(), style: GutterStyle::Normal });
    }
    match ctx.annotations.diagnostic_severity {
        4 => Some(GutterCell { text: "E".into(), style: GutterStyle::Normal }),
        3 => Some(GutterCell { text: "W".into(), style: GutterStyle::Normal }),
        2 => Some(GutterCell { text: "I".into(), style: GutterStyle::Normal }),
        1 => Some(GutterCell { text: "H".into(), style: GutterStyle::Dim }),
        _ => None,
    }
});
```

---

## Phase 3: Rendering Integration

### Changes to `crates/api/src/render/buffer/context.rs`

**Remove**: Hardcoded gutter rendering in `render_buffer()` (lines 247-278, 430-436, 478-490).

**Add**: Gutter compositor that iterates enabled columns:

```rust
use xeno_registry_gutter::{
    GUTTERS, GutterLineContext, GutterWidthContext, GutterWidth,
    GutterStyle, GutterAnnotations, enabled_gutters, total_width,
};

impl BufferRenderContext<'_> {
    /// Renders gutter for a single line, returning styled spans.
    fn render_gutter_line(
        &self,
        ctx: &GutterLineContext,
        gutter_widths: &[(u16, &'static GutterDef)],
    ) -> Vec<Span> {
        let mut spans = Vec::new();
        
        for (width, gutter) in gutter_widths {
            let cell = (gutter.render)(ctx);
            let (text, style) = match cell {
                Some(c) => {
                    let fg = match c.style {
                        GutterStyle::Normal => self.theme.colors.ui.gutter_fg,
                        GutterStyle::Dim => self.theme.colors.ui.gutter_fg
                            .blend(self.theme.colors.ui.bg, 0.5),
                        GutterStyle::Cursor => self.theme.colors.ui.gutter_fg,
                    };
                    let mut s = Style::default().fg(fg);
                    if ctx.is_cursor_line {
                        s = s.bg(self.theme.colors.ui.cursorline_bg);
                    }
                    (format!("{:>w$}", c.text, w = *width as usize), s)
                }
                None => {
                    (" ".repeat(*width as usize), Style::default())
                }
            };
            spans.push(Span::styled(text, style));
        }
        
        // Trailing separator space
        spans.push(Span::raw(" "));
        spans
    }
}
```

---

## Phase 4: Replace `gutter_width()` Calls

### Files to Modify

| File | Change |
|------|--------|
| `crates/api/src/buffer/mod.rs` | Remove `gutter_width()` method |
| `crates/api/src/render/buffer/context.rs` | Use `total_width()` from registry |
| `crates/api/src/render/buffer/viewport.rs` | Use `total_width()` from registry |
| `crates/api/src/buffer/navigation.rs` | Use `total_width()` from registry |
| `crates/api/src/editor/focus.rs` | Use `total_width()` from registry |
| `crates/api/src/editor/buffer_manager.rs` | Use `total_width()` from registry |
| `crates/api/src/editor/lifecycle.rs` | Use `total_width()` from registry |
| `crates/api/src/editor/input/mouse_handling.rs` | Use `total_width()` from registry |
| `crates/api/src/capabilities.rs` | Use `total_width()` from registry |
| `crates/api/src/editor/navigation.rs` | Remove `gutter_width()` delegation |

### Helper Function

Add to `crates/api/src/buffer/mod.rs` or a new `gutter.rs` module:

```rust
/// Computes current gutter width for this buffer.
pub fn gutter_width_for_buffer(buffer: &Buffer) -> u16 {
    let ctx = GutterWidthContext {
        total_lines: buffer.doc().content.len_lines(),
        viewport_width: buffer.text_width as u16 + 100, // approximate
    };
    xeno_registry_gutter::total_width(&ctx)
}
```

---

## Phase 5: Workspace Integration

### Files to Modify

| File | Change |
|------|--------|
| `crates/registry/Cargo.toml` | Add `gutter` member |
| `crates/registry/src/lib.rs` | Re-export `xeno_registry_gutter` |
| `crates/api/Cargo.toml` | Add dep on `xeno-registry-gutter` |
| `crates/term/Cargo.toml` | Add dep on `xeno-registry-gutter` (for `use _ as _` force-link) |

### Re-export

```rust
// crates/registry/src/lib.rs
pub use xeno_registry_gutter as gutter;
```

---

## Task Checklist

### Phase 1: Registry Crate
- [ ] Create `crates/registry/gutter/Cargo.toml`
- [ ] Create `crates/registry/gutter/src/lib.rs` with core types
- [ ] Create `crates/registry/gutter/src/macros.rs` with `gutter!` macro
- [ ] Add to workspace in `crates/registry/Cargo.toml`

### Phase 2: Built-in Implementations
- [ ] Create `crates/registry/gutter/src/impls/mod.rs`
- [ ] Implement `line_numbers` (absolute)
- [ ] Implement `relative_line_numbers`
- [ ] Implement `hybrid_line_numbers`
- [ ] Implement `signs` column

### Phase 3: Rendering Integration
- [ ] Add `xeno-registry-gutter` dependency to `xeno-api`
- [ ] Create `render_gutter_line()` helper in `BufferRenderContext`
- [ ] Refactor `render_buffer()` to use gutter compositor
- [ ] Handle empty lines (`~` indicator) via gutter or special case
- [ ] Verify cursor line highlighting works with new system

### Phase 4: Replace Hardcoded Calls
- [ ] Replace `buffer.gutter_width()` in `render/buffer/context.rs`
- [ ] Replace `buffer.gutter_width()` in `render/buffer/viewport.rs`
- [ ] Replace `buffer.gutter_width()` in `buffer/navigation.rs`
- [ ] Replace `buffer.gutter_width()` in `editor/focus.rs`
- [ ] Replace `buffer.gutter_width()` in `editor/buffer_manager.rs`
- [ ] Replace `buffer.gutter_width()` in `editor/lifecycle.rs`
- [ ] Replace `buffer.gutter_width()` in `editor/input/mouse_handling.rs`
- [ ] Replace `buffer.gutter_width()` in `capabilities.rs`
- [ ] Remove `gutter_width()` from `Buffer` and `Editor`

### Phase 5: Finalization
- [ ] Add re-export in `crates/registry/src/lib.rs`
- [ ] Force-link in `xeno-term` main
- [ ] Run tests, fix any breakage
- [ ] Update AGENTS.md registry table

---

## Future Extensions

This design enables:

| Extension | Gutter | Priority | Width |
|-----------|--------|----------|-------|
| LSP diagnostics | `diagnostics` | -10 | Fixed(2) |
| Git hunks | `git_status` | -5 | Fixed(1) |
| Breakpoints | `breakpoints` | -15 | Fixed(2) |
| Fold indicators | `folds` | 5 | Fixed(1) |
| Blame | `blame` | 10 | Dynamic |

Each is a simple `gutter!()` registration. Annotations populate `GutterAnnotations` from LSP/git data.

---

## Notes

- **Empty line indicator** (`~`): Could be a separate gutter column with very high priority, or handled as special case in renderer. Leaning toward special case since it only appears past EOF.

- **Performance**: Closures are `fn` pointers, not `dyn Fn`, so zero allocation per call. Width computation happens once per render, not per line.

- **Mutually exclusive columns**: `line_numbers`, `relative_line_numbers`, and `hybrid_line_numbers` all have priority 0. Only one should be enabled at a time. Config validation could enforce this, or the first enabled wins.
