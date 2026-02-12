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

## Refactor execution tracker

Execution order: seam-first option `2 -> 1 -> 3`.

- [x] Runtime/input seam introduced (`EditorFrontend` + `EditorEngineOps` traits, terminal loop moved behind frontend boundary).
- [x] Overlay internals hidden behind accessors and notification seam introduced.
- [x] Terminal runtime/backend ownership moved to `xeno-editor-tui`.
- [x] Geometry backing moved to `xeno-primitives` (`Rect`/`Position`) with explicit frontend conversion boundaries.
- [x] Frame composition loop moved to `xeno-editor-tui` (`run_editor` now renders through `xeno_editor_tui::compositor`).
- [x] Completion/snippet/info-popup layer orchestration moved to `xeno-editor-tui` (editor exports visibility/render helpers).
- [ ] Move `ui/*`, `render/*`, and `info_popup` ownership to `xeno-editor-tui`.
- [ ] Make `xeno-editor` build headless with `xeno-tui` optional and verify with `--no-default-features`.
