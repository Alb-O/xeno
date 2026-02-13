# iced_wgpu integration notes

Date: 2026-02-13

## Scope

Investigate a minimal GUI frontend integration using `iced_wgpu` while preserving existing core/frontend boundaries.

## Implemented prototype

- Added `xeno-editor-iced` crate (`crates/editor-iced`).
- Added feature gate `iced-wgpu` so the crate can exist without forcing iced compilation in default workspace builds.
- Added `xeno-iced` binary (`cargo run -p xeno-editor-iced --features iced-wgpu --bin xeno-iced`).
- Wired core runtime loop into iced:
  - `Editor::pump()` and `Editor::on_event(RuntimeEvent)` are driven by a local tokio runtime.
  - lifecycle hooks are emitted on startup and quit.
- Added event bridge from iced -> core runtime:
  - keyboard key presses (character + named keys + modifiers)
  - window opened/resized
  - window focus/unfocus
- Added minimal rendering bridge:
  - renders focused buffer as a plain text snapshot
  - renders statusline using `statusline_render_plan`
  - intentionally does not reuse TUI widget/render backend

## Current limitations

- Geometry mismatch:
  - core currently treats `RuntimeEvent::WindowResized { width, height }` as terminal-like cell dimensions.
  - iced provides logical pixel sizes; prototype currently passes rounded pixel values directly.
- Rendering seam is still provisional:
  - no style/span-level GUI renderer yet
  - no overlay/completion/snippet/info-popup scene rendering yet
- Input coverage is partial:
  - no mouse adapter
  - no clipboard paste adapter
  - no IME/text composition adapter
- Dependency wiring is local checkout based:
  - crate uses local `../iced` path checkout for `iced` dependency.
- Linux display backend quirks:
  - `xeno-editor-iced` enables both `x11` and `wayland` build features.
  - runtime now auto-selects Wayland first when available to avoid hard X11 runtime dependency on Wayland-only systems.
  - `XENO_ICED_BACKEND={wayland|x11}` can be used to force backend choice.

## Best next iteration

1. Introduce a GUI geometry contract in core runtime:
   - separate logical pixels from terminal cells in resize handling.
2. Add backend-neutral scene data for document text/style + overlay surfaces:
   - consume the same core plans in both TUI and iced frontends.
3. Replace snapshot rendering with plan-driven GUI rendering:
   - start with document + statusline, then completion/snippet/overlay panes.
4. Extend event adapter:
   - mouse mapping
   - paste/clipboard mapping
   - IME composition path
5. Add replay tests shared across frontends:
   - verify equal core state transitions/plan outputs under identical event scripts.
