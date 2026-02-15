# Architecture core issue: current state summary

Date: 2026-02-13

## Purpose

This document tracks the architecture split between `xeno-editor` (core engine) and `xeno-frontend-tui` (frontend/runtime), with a concise status view instead of per-commit logs.

## Target model

* `xeno-editor` owns editor state, document operations, policies, effects, and backend-neutral render data.
* `xeno-frontend-tui` owns terminal runtime, frame composition, widget rendering, and frontend interaction wiring.
* Engine/frontend coordination happens through data APIs and explicit seams, not internal field reach-through.

## Current state

### Ownership

* Frontend runtime/composition is in `xeno-frontend-tui`.
* Core now owns behavior/data planning for completion, snippet choice, modal overlay panes, utility which-key, and statusline composition.
* Frontend rendering remains toolkit-specific (`xeno-frontend-tui` owns widget composition, style mapping, and frame layering).

### Boundary hardening completed

* `xeno-editor` no longer has direct `xeno_tui` usage in `crates/editor/src` (current count: `0`).
* `xeno-editor` no longer publicly exposes broad internal modules:
  * `render` is internal; frontend imports via `render_api`.
  * `impls` is internal; focused symbols are re-exported at crate root where needed.
  * `notifications` internals are hidden behind typed editor APIs.
* Non-`impls` modules now reference the public `crate::Editor` path instead of `crate::impls::Editor`, reducing internal-path coupling in public-facing APIs/docs.
* Core render text primitives (`RenderLine`/`RenderSpan`) are backend-neutral and exposed through `render_api` for explicit frontend adaptation.
* Frontend no longer reaches through overlay internals for policy:
  * no `overlay_interaction()` usage in frontend crates.
  * no frontend overlay store reads for completion/snippet/status/utility behavior.
  * frontends consume typed plans/APIs (`overlay_pane_render_plan`, `whichkey_render_plan`, `statusline_render_plan`, `completion_render_plan`, `snippet_choice_render_plan`).
* `xeno-frontend-tui` now consumes lifecycle hook and notification render surfaces through `xeno-editor` APIs and has no direct `xeno_registry` imports.
* `xeno-term` startup config loading/application also routes through `xeno-editor`, and `xeno-term` no longer imports `xeno_registry` directly.
* Runtime resize contract is now explicitly text-grid based (`cols`/`rows`) with frontend-side pixel-to-grid adaptation for GUI frontends.
* `xeno-frontend-iced` now consumes core completion/snippet/overlay/info-popup plans (rendered as a structured scene summary while native GUI widgets are still in progress).
* `xeno-frontend-iced` document snapshot path now runs through core `BufferRenderContext` instead of directly reading buffer text.
* `xeno-frontend-iced` now maps mouse cursor/button/scroll events into core `MouseEvent` coordinates using the shared grid conversion path.
* `xeno-frontend-iced` now routes Command/Ctrl+V through `RuntimeEvent::Paste` using iced clipboard read tasks.
* `xeno-frontend-iced` now routes IME commit text into the same core paste path (`RuntimeEvent::Paste`).
* `xeno-frontend-iced` now renders completion/snippet plan rows as dedicated previews (beyond simple visibility summaries), increasing plan-level frontend parity.
* `xeno-frontend-iced` now tracks IME preedit lifecycle in frontend state (snapshot header) for composition observability while core event modeling remains commit-focused.
* `xeno-frontend-iced` snapshot data now preserves typed overlay/completion/snippet/info-popup plan outputs and leaves inspector row formatting to the frontend adapter layer.
* `xeno-frontend-iced` inspector now renders directly from typed surface plans without intermediate row-model shims.
* `xeno-frontend-iced` completion/snippet inspector rows now use typed part mapping before widget composition (instead of string-concatenated rows).
* `xeno-frontend-iced` snapshot header is now typed data (`HeaderSnapshot`) with frontend-local formatting, reducing stringly seams.
* Statusline style-color policy is now centralized in `xeno-editor` (`statusline_segment_style`) and consumed by both TUI and iced adapters to reduce frontend drift.
* Runtime replay test coverage now includes equivalent event-script convergence checks for single-line and multiline input paths (paste vs typed text/Enter) for core state/statusline outputs.
* Runtime replay coverage now also includes command-palette input convergence (paste vs typed keys) for completion-plan, overlay pane role/geometry, and statusline equivalence.
* Runtime replay coverage now includes search-overlay input convergence (paste vs typed keys), including overlay pane role/geometry equivalence checks.
* Legacy compatibility paths/shims removed:
  * focus compatibility helper path removed in favor of unified `set_focus` flow.
  * redundant focus hook `ViewId` adapter shim removed (hooks now receive canonical view IDs directly).
  * unused overlay compatibility constructor argument removed.
  * stale render seam exports and dead helper surface removed.
* Umbrella passthrough patterns were reduced:
  * callsites now import types from owning modules (`types`, `command_queue`, etc.).
* Public overlay helper surface was pruned (`overlay_pane_count` removed) in favor of plan-based consumers.
* Overlay store accessors are now crate-private to prevent new frontend reach-through.
* Plan-builder regression tests now cover completion, snippet-choice, and statusline policy outputs.

### Feature posture

* `xeno-editor` supports headless builds (`--no-default-features`).
* `tui` remains an optional feature on `xeno-editor` (default-enabled), with frontend runtime in `xeno-frontend-tui`.
* `xeno-editor` declares `tui -> xeno-primitives/xeno-tui` feature mapping; fully removing transitive `xeno-tui` from headless builds is a remaining workspace-level follow-up.
* `xeno-frontend-iced` exists as an experimental frontend crate behind `iced-wgpu` feature for GUI runtime probing without changing core ownership boundaries.

## Remaining work

* Decide whether panel registration/state should remain in core `UiManager` or move fully to frontend ownership.
* Finish backend-neutral text/style seam tightening (`Style`/`Line`/`Span` render boundary) for future GUI adapters.
* Port the experimental iced frontend from text snapshot rendering to full plan/render adapters shared with TUI policy outputs.
* Add replay/snapshot coverage for plan builders to guard cross-frontend behavior consistency.
* Keep pruning legacy/compat patterns as they appear during refactors.

## Acceptance checks

Use this matrix for boundary-sensitive changes:

```bash
cargo check -p xeno-editor --all-targets
cargo check -p xeno-editor --no-default-features --all-targets
cargo check -p xeno-frontend-tui
cargo check -p xeno-term
```

Run targeted tests for touched areas (`focus`, `mouse`, `row`, panel/overlay tests) before merge.

## History policy

Detailed checkpoint history is intentionally kept in Git commit history and PRs.

Useful resume commands:

```bash
git log --oneline -n 40
rg -n "xeno_tui::|use xeno_tui" crates/editor/src -S
rg -n "xeno_editor::render::|xeno_editor::impls::|xeno_editor::notifications::" crates/frontend-tui/src crates/term/src -S
```
