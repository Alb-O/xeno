### What changed (high-level)

We eliminated the main “policy-in-frontend” drift vectors:

* **Document rendering**: frontends no longer assemble `BufferRenderContext` or touch core caches/diagnostics/syntax.
* **Overlay completion menu geometry** (command palette/file picker): now computed in core, bounded by core’s utility overlay sizing policy.
* **Statusline row policy**: now a core API (`Editor::statusline_rows()`), not frontend constants.
* **Info popup geometry + sizing caps**: now computed in core (`info_popup_layout_plan(bounds)`), not in TUI.

### Current seam contract (core → frontends)

Frontends should treat these as the only render/layout inputs:

* `Editor::focused_document_render_plan() -> DocumentRenderPlan`
* `Editor::buffer_view_render_plan(view, area, use_block_cursor, is_focused) -> Option<BufferViewRenderPlan>`
* `Editor::buffer_view_render_plan_with_gutter(..., gutter) -> Option<BufferViewRenderPlan>`
* `Editor::overlay_completion_menu_target() -> Option<OverlayCompletionMenuTarget>`
* `Editor::statusline_render_plan() -> Vec<StatuslineRenderSegment>`
* `Editor::statusline_rows() -> u16`
* `Editor::info_popup_layout_plan(bounds) -> Vec<InfoPopupLayoutTarget>`
* Existing plans: overlay panes, snippet choice, completion popup, which-key, statusline styles, etc.

**Key invariant:** frontend crates should not be able to recreate core render assembly. `render_api` no longer exports the internal context types.

### Repo hygiene checks (run before/after any UI work)

```bash
# hard boundary grep: should stay empty in frontend crates
rg -n "BufferRenderContext|RenderCtx|RenderBufferParams" crates/editor-tui/src crates/editor-iced/src -S

cargo fmt
cargo test -p xeno-editor
cargo check -p xeno-editor --no-default-features --all-targets
cargo test -p xeno-editor-tui
cargo test -p xeno-editor-iced --features iced-wgpu
cargo check -p xeno-term
```

### Next incremental tickets (recommended order)

#### Ticket 6: Trim remaining `render_api` exports (if unused)

`render_api.rs` still exports `GutterLayout` + `ensure_buffer_cursor_visible`. These are policy-adjacent and ideally should not be public.

* Do:

  * `rg -n "GutterLayout|ensure_buffer_cursor_visible" crates/editor-tui/src crates/editor-iced/src -S`
  * If empty, remove from `render_api.rs`.
* Acceptance: same matrix as above.

#### Ticket 7: Iced renders real overlay panes + info popups (stop “inspector-only”)

Now that core provides:

* overlay pane plan (rect + content_rect + gutter)
* `buffer_view_render_plan_with_gutter`
* `info_popup_layout_plan(bounds)`
  …iced can render:
* overlay panes (input/list/preview) using pane `content_rect`
* info popups using layout targets’ rect + `Hidden` gutter
  This is the natural next correctness step for GUI parity.

#### Ticket 8: Info popup `PopupAnchor::Window` semantics

Today `PopupAnchor::Window(_)` maps to `Center`. If desired, implement real window-adjacent anchoring in core layout plan (still keep frontends dumb).

#### Ticket 9: Replay/integration convergence tests (cross-frontend)

Add runtime replay scripts asserting that identical event sequences produce equivalent:

* overlay kind/panes
* completion menu targets
* statusline segments
* info popup layout targets

### Notes / known non-issues

* Upstream `iced_winit` unused-import warning persists; optional cleanup-only ticket if you want zero warnings.

If you want a final “cleanup-only” ticket next: do Ticket 6 (render_api trimming) + warning cleanup in one pass, and keep the acceptance matrix unchanged.
