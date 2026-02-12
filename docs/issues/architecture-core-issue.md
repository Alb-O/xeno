## 1) The “Editor” type is doing three jobs at once

The current `Editor`/`EditorApp` was simultaneously:

* **Engine/state owner** (documents, undo, workspace, async systems)
* **UI state owner** (overlays, panels, notifications, render cache)
* **Frontend runtime** (terminal events, key/mouse dispatch, tick/pump loop)

Because the same concrete type owned all three, almost every module implicitly assumed it could reach:

* `self.state.*` (engine internals)
* `self.ui.*` (UI internals)
* `termina/xeno_tui` types (backend specifics)

This makes any crate split non-local: “move one file” immediately breaks dozens of call sites because those call sites were relying on the monolith.

## 2) Inherent impl locality makes partial moves impossible

Rust rule: **inherent `impl Type {}` must be in the crate that defines `Type`.**

You hit this hard when you tried to move `impl EditorApp` methods (`flush_effects`, `open_*`, `show_notification`, interaction helpers) to `xeno-editor-tui` while `EditorApp` was still defined in `xeno-editor`. That can’t work without duplicating code or moving the type itself.

This is why “bounded moves” kept failing: you can’t move “just the methods” that everyone calls.

## 3) Privacy + deep field access was the real coupling

The entire app layer accessed `EditorEngine.state` and `EditorApp.ui` *directly* across many modules.

Even after you introduced `EngineView/EngineViewMut`, you discovered the bigger issue: there were **hundreds** of `.state.*` reads/writes spread across lifecycle/input/runtime/invocation/UI. That meant:

* Moving any of those modules to another crate required either

  * exposing a large public surface (bad), or
  * refactoring them all to use a façade (large but doable), or
  * moving them together with the type (big-bang).

So the real coupling wasn’t Cargo deps—it was “struct field reach-through everywhere”.

## 4) Effects/side-effects were not a stable contract

The “flush effects” mechanism is the natural seam between engine and UI, but initially:

* effects were being **applied inside engine modules** (or required app callbacks),
* snippet session behavior required **synchronous** application semantics,
* nested edits/hook-driven edits triggered **reentrancy hazards** (you saw hanging tests).

You eventually stabilized this by:

* making effects engine-pure (emit only),
* applying effects in the app,
* guarding flush depth.

The lesson: the engine/UI boundary needs a **real event/effect contract** that is:

* pure data,
* deterministic ordering,
* safe under reentrancy,
* explicitly “apply now” vs “defer”.

Without that, moving editing logic between engine/app causes behavioral regressions.

## 5) UI subsystems weren’t encapsulated

Overlays, render cache, and UI manager had callers that reached into their internals:

* `overlay_system.layers`
* `overlay_system.interaction.active.session...`
* direct cache epoch fiddling
* direct store/controller pokes

Those internals are exactly what a frontend crate should own. But because the engine crate exposed and used them directly, any split required either:

* wrapping them with façade methods (public API on overlay/cache/ui manager), or
* moving everything that touches them in one shot.

You started down the façade path for overlays—good direction—but it’s widespread.

## 6) Terminal backend types leaked into “core” paths

Even with `xeno-input` present, a lot of “core-ish” logic still referenced `termina`/TUI geometry/layout or assumed a terminal frame loop. That violates the desired layering:

* Backend events → should be mapped to stable input types
* Engine → should be backend-agnostic
* TUI → should be the only place that knows about terminal widgets and rendering

This leakage makes it hard to define an engine crate that is genuinely headless.

## 7) The dependency graph reflects intent, but the code graph doesn’t

Your manifest findings suggested a plausible layering, but the *implementation* had cross-cutting modules:

* lifecycle/tick/pump mixed engine maintenance with UI animation and overlay behavior
* invocation mixed capability checking with notification display and hook emission
* input handling mixed UI focus/panels with engine edit actions and overlay dispatch

So even if crates are arranged “correctly”, the module boundaries aren’t aligned with them.

---

# Practical takeaways for a fresh “ground-up” attempt

## A) Define stable interfaces first (before moving files)

1. **Engine API**: public methods + read/write views (like `EngineView/EngineViewMut`) but keep it small and intentional.
2. **Effect contract**: a single `Effect`/`Event` enum that represents *everything* UI must do as a result of engine operations.
3. **Overlay/UI façade**: overlay manager should expose methods like `open(kind,args)`, `handle_event`, `apply_request`, not internals.

## B) Make “EditorApp” not be an engine impl target

Instead of `impl EditorApp { ... }` everywhere, define traits:

* `EditorFrontend` (UI + input + render operations)
* `EditorEngineOps` (engine operations)

Then your UI crate implements the frontend trait, and the engine exposes ops. This avoids the inherent-impl locality trap.

## C) Pick a single seam and enforce it

The seam that worked best in this session was:

* engine emits effects
* app applies effects
* app decides timing (`flush_effects`)

Build everything else around that.

## D) Move by ownership, not by “which file imports xeno_tui”

The stable move unit isn’t “files importing xeno_tui”. It’s:

* the **type ownership boundary**: whoever defines `EditorApp` must own all its inherent impls.
  So if `EditorApp` stays in engine crate, UI impls cannot move. If UI impls must move, `EditorApp` must move too (or become a trait object).

---

If you start a fresh session, the most important “north star” statement you can paste is:

> “We want an engine crate with no terminal/UI deps, and a frontend crate that owns rendering, overlays, and input. The engine emits pure data effects; the frontend applies them deterministically. No module outside the engine crate may access engine internals; no module outside the frontend crate may access overlay/ui internals.”

Everything above is essentially evidence that the current code violates that statement in many places, and partial moves fail because of Rust’s inherent impl and privacy rules.

## Refactor execution tracker (living handoff)

Execution order chosen: seam-first option `2 -> 1 -> 3`.

### Snapshot for next pickup

- Date of this snapshot: `2026-02-12`.
- Branch at snapshot: `main`.
- Last refactor commit in this chain: `91ab8e20`.
- Working tree at snapshot end: clean.
- High-level state: runtime/composition ownership is now in `xeno-editor-tui`; legacy editor-side composition path is deleted.

### Commit map (chronological)

| Commit | Scope | Result |
| --- | --- | --- |
| `45f5d4f6` | runtime event mapping | terminal event mapping moved to term boundary |
| `31390aaf` | overlay encapsulation | overlay internals hidden behind accessors |
| `f49956e5` | temporary runtime seam | introduced `EditorFrontend`/`EditorEngineOps` |
| `2ada1033` | geometry seam | routed core layout/overlay rects through geometry module |
| `25bed5ae` | notifications seam | wrapped toast manager behind notification center |
| `fbcb4c2e` | frontend crate split | terminal runtime/backend moved to `xeno-editor-tui` |
| `221ef13a` | primitives geometry | canonical `Rect`/`Position` moved to `xeno-primitives` |
| `7708689b` | frontend composition | `run_editor` renders through `xeno_editor_tui::compositor` |
| `2be960b6` | popup layer ownership | completion/snippet/info-popup layer orchestration moved to frontend |
| `0de3258f` | scene primitives ownership | `SceneBuilder`/`UiScene` moved to frontend crate |
| `92835a38` | document rendering ownership | split document render orchestration moved to frontend crate |
| `5fd8f73b` | seam cleanup | removed temporary `EditorFrontend`/`EditorEngineOps` traits |
| `91ab8e20` | dead path removal | deleted legacy editor compositor + document module + popup layer modules |

Note: `f49956e5` was an intentional stepping stone and is now superseded by `5fd8f73b`.

### Completed milestones

- [x] Terminal runtime/backend ownership moved to `xeno-editor-tui`.
- [x] Geometry canonicalized in `xeno-primitives`.
- [x] Frame composition loop moved to `xeno-editor-tui`.
- [x] Completion/snippet/info-popup layer orchestration moved to `xeno-editor-tui`.
- [x] Scene/layer primitives moved to `xeno-editor-tui`.
- [x] Split document rendering orchestration moved to `xeno-editor-tui`.
- [x] Temporary runtime traits removed after ownership transfer.
- [x] Legacy editor-side compositor/document/popup-layer modules removed.

### Current ownership map

- Frontend-owned (`xeno-editor-tui`):
  - terminal runtime loop (`crates/editor-tui/src/lib.rs`)
  - compositor (`crates/editor-tui/src/compositor.rs`)
  - scene/layer primitives (`crates/editor-tui/src/scene.rs`, `crates/editor-tui/src/layer.rs`)
  - popup layers (`crates/editor-tui/src/layers/*`)
  - split document rendering (`crates/editor-tui/src/document/*`)
- Core/editor-owned (`xeno-editor`):
  - editor state, layout, input, overlay managers
  - `ui/*` manager/panel state and modal overlay render path used by utility panel
  - most `render/*` internals (buffer context/cache/text shaping/wrap/status/completion menu widgets)
  - info popup state + rect/style helpers in `crates/editor/src/info_popup/mod.rs`

### Remaining work for option 2 (ownership-first)

- [ ] Move remaining `ui/*` ownership to `xeno-editor-tui`.
- [ ] Move remaining `render/*` ownership to `xeno-editor-tui`.
- [ ] Move `info_popup` ownership to `xeno-editor-tui`.
- [ ] Make `xeno-editor` build headless by making `xeno-tui` optional and passing `--no-default-features`.

### Concrete hotspots still coupling editor to TUI

As of this snapshot, `crates/editor/src` still has ~100 `xeno_tui` references.

Highest-density files:
- `crates/editor/src/render/completion.rs`
- `crates/editor/src/render/snippet_choice.rs`
- `crates/editor/src/ui/layers/modal_overlays.rs`
- `crates/editor/src/ui/panels/utility.rs`
- `crates/editor/src/overlay/geom.rs`
- `crates/editor/src/ui/manager.rs`

Directories still in editor and expected to shrink/move:
- `crates/editor/src/ui/*`
- `crates/editor/src/render/*`
- `crates/editor/src/info_popup/*`

### Known constraints and traps

- `UiManager` still owns panel state and renders panels directly; utility panel render path calls modal overlay rendering (`ui/layers/modal_overlays.rs`), so UI ownership is not fully moved yet.
- `editor-tui` currently imports render/info types from `xeno-editor` (for example `xeno_editor::render::{BufferRenderContext, RenderCtx}` and `xeno_editor::info_popup::*`), meaning ownership transfer is incomplete.
- `LayerId::new` and layout slot accessors were opened for frontend rendering orchestration; keep API use intentional and revisit if a stricter facade is introduced.
- Avoid reintroducing the removed legacy path (`editor::ui::compositor`, `editor::render::document`, `editor::ui::layers::{completion,info_popups,snippet_choice}`).

### Recommended pickup plan (next session)

1. Move panel rendering out of `UiManager` into frontend ownership.
2. Replace editor-side panel render calls with data-only render plans.
3. Move `modal_overlays` rendering module to `xeno-editor-tui` and call from frontend panel renderer.
4. Move remaining render orchestration entrypoints (`render/status`, completion menu widget composition, snippet choice menu widget composition) into `xeno-editor-tui`.
5. Move info popup style/rect compute helpers and store operations to frontend boundary where appropriate.
6. Gate `xeno-editor` TUI-dependent modules behind a feature and pass `cargo check -p xeno-editor --no-default-features`.

### Acceptance checks at each future breakpoint

- `cargo check -p xeno-editor`
- `cargo check -p xeno-editor-tui`
- `cargo check -p xeno-term`
- targeted tests relevant to touched modules
- final headless checkpoint:
  - `cargo check -p xeno-editor --no-default-features`
  - `cargo check -p xeno-term`

### Quick resume commands

```bash
git status --short --branch
git log --oneline -n 20
rg -n "xeno_tui::|use xeno_tui" crates/editor/src -S | wc -l
rg -n "xeno_editor::render|xeno_editor::ui::|xeno_editor::info_popup" crates/editor-tui/src -S
```
