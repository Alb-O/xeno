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
  - mouse cursor/button/scroll events mapped from measured document viewport space into core `MouseEvent` grid coordinates
  - optional coordinate normalization for platform scaling quirks (`XENO_ICED_COORD_SCALE`, `XENO_ICED_COORD_SCALE_X`, `XENO_ICED_COORD_SCALE_Y`)
  - clipboard paste bridge for Command/Ctrl+V via `iced::clipboard::read_text()`
  - IME commit bridge (`input_method::Event::Commit`) routed into core paste path
  - IME lifecycle tracking for opened/preedit/closed state (surfaced in iced snapshot header)
  - document viewport size sensor drives core `WindowResized` grid updates
  - window focus/unfocus
  - adapter-level unit tests now cover IME commit/preedit event mapping
- Added minimal rendering bridge:
  - renders focused buffer snapshot via core `BufferRenderContext` (shared render-policy path)
  - preserves core `RenderLine` rows and adapts them to iced `rich_text` rows (span-level foreground/background mapping for named/RGB/indexed colors)
  - document/statusline rendering keeps iced default text sizing/line-height behavior; input mapping calibration is handled separately
  - renders statusline using `statusline_render_plan`
  - consumes core completion/snippet/overlay/info-popup plans and shows a structured scene summary
  - renders completion/snippet plan rows as dedicated preview sections with semantic row roles (meta/normal/selected)
  - applies theme-driven container backgrounds for app/document/inspector surfaces (instead of toolkit default white)
  - uses split layout (document + inspector column) to avoid debug sections pushing document content downward
  - supports runtime layout tuning (`XENO_ICED_INSPECTOR_WIDTH_PX`, `XENO_ICED_SHOW_INSPECTOR`)
  - intentionally does not reuse TUI widget/render backend

## Current limitations

- Grid-size conversion is heuristic:
  - core resize contract is grid-based (`RuntimeEvent::WindowResized { cols, rows }`) and now sourced from measured document viewport size.
  - iced maps logical pixels to cols/rows via configurable cell metrics (`XENO_ICED_CELL_WIDTH_PX`, `XENO_ICED_CELL_HEIGHT_PX`) with a fixed +1 statusline row reserve.
  - no font-metrics-driven calibration yet.
- Rendering seam is still provisional:
  - style/span mapping is currently color/background focused; advanced text-style parity remains incomplete
  - overlay/completion/snippet/info-popup plans are wired, but currently rendered as textual diagnostics instead of native GUI surfaces
- Input coverage is partial:
  - paste adapter currently handles Command/Ctrl+V; no broader clipboard event coverage yet
  - IME preedit/lifecycle is currently frontend-local diagnostics state, not a first-class core runtime event
- Dependency wiring is local checkout based:
  - crate uses local `../iced` path checkout for `iced` dependency.
- Linux display backend quirks:
  - `xeno-editor-iced` enables both `x11` and `wayland` build features.
  - runtime now auto-selects Wayland first when available to avoid hard X11 runtime dependency on Wayland-only systems.
  - `XENO_ICED_BACKEND={wayland|x11}` can be used to force backend choice.

## Resize crash investigation (WSLg)

Environment and scope:
- this crash is currently observed specifically under WSLg.
- startup stability was best with `XENO_ICED_BACKEND=x11` and `WGPU_BACKEND=vulkan`.
- failure appears during interactive window resize, not at deterministic startup in the stable variant above.

Observed runtime output near failure:
- repeated `libEGL` / `MESA-LOADER` warnings (driver/device lookup failures)
- `MESA: error: ZINK: failed to choose pdev`
- `libEGL warning: egl: failed to create dri2 screen`
- `Io error: Broken pipe (os error 32)`

Reproduction profile from manual testing:
- empty or small documents were generally stable.
- long single-line document was stable (isolated soft-wrap path).
- many short lines were stable.
- crashes were more reproducible with larger multi-line documents.
- issue reproduces even without syntax highlighting and on plain-text files (for example `THIRD_PARTY_NOTICES`), so syntax policy is unlikely to be the primary trigger.

Mitigations attempted and reverted/reset after no improvement:
- resize-path guardrails in frontend adapter (duplicate resize suppression and backend retry logic).
- snapshot fallback reduction during transient invalid viewport states.
- per-frame document row render cap in iced document adapter.
- additional broken-pipe detection/failover logging around run loop.

Current conclusion:
- evidence points to a WSLg graphics stack/runtime interaction (`EGL`/Mesa/Zink + backend pipe teardown) rather than a single deterministic core editor policy bug.

## Best next iteration

1. Calibrate GUI grid mapping:
   - replace fixed pixel-per-cell defaults with measured font metrics.
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
