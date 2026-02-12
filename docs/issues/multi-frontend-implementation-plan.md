# Multi-frontend implementation plan (TUI + GUI)

Date: 2026-02-12

## Goal

Support multiple frontend implementations (TUI and GUI) with:

- minimal duplicated behavior logic
- consistent user-visible behavior across frontends
- frontend freedom for toolkit-specific rendering and platform integration

## Decision

Use a data-plan architecture:

- `xeno-editor` owns behavior policy and emits frontend-facing data plans.
- each frontend crate (`xeno-editor-tui`, future `xeno-editor-gui`) maps plans to native rendering/widgets.
- frontend crates own event loop, platform IO, and rendering backend details.

## Why this path

Current code already has strong foundations:

- frontend-agnostic event/runtime contract in core (`RuntimeEvent`, `pump`, `on_event`)
- existing data-plan seams (`panel_render_plan`, `info_popup_render_plan`)

Largest remaining duplication risk is behavior policy still living in TUI render modules (placement rules, popup sizing, overlay logic, statusline composition). If copied to GUI, behavior drift is likely.

## Architecture target

### Core (`xeno-editor`)

Own:

- editor state, mode, buffers, overlays, policies
- effect/event ordering and state transitions
- frontend-facing data plans for every major surface

Expose:

- input/event API (`RuntimeEvent`, `on_event`, `pump`)
- scene/view-model plans
- typed UI intents and overlay metadata (no direct access to overlay internals)

### Frontends (`xeno-editor-tui`, `xeno-editor-gui`)

Own:

- platform event loop, windowing, clipboard/IME specifics
- rendering and widget composition for their toolkit
- backend-specific animation/chrome implementation

Consume:

- core plans and runtime API only

## Migration plan

### Phase 1: expand core plans for remaining behavior hotspots

Add data-plan APIs in core for:

- completion popup
- snippet choice popup
- modal overlay presentation
- statusline composition
- utility/which-key panel content

Definition of done:

- TUI no longer computes placement/visibility/sizing policy for these surfaces.
- TUI only draws from plans.

### Phase 2: remove frontend access to overlay internals

Replace frontend usage of controller/session internals with typed plan outputs:

- overlay kind
- pane list and content rects
- focused pane
- completion/menu anchors

Definition of done:

- no `overlay_interaction().active().controller/session` policy reads in frontend.

### Phase 3: finalize backend-neutral text/style seam

Replace remaining backend-coupled style/text aliases with canonical backend-neutral render data.

Definition of done:

- core render outputs are toolkit-agnostic.
- each frontend performs adapter mapping to its own primitives.

### Phase 4: add GUI frontend crate

Create `xeno-editor-gui` with:

- GUI event adapter -> `RuntimeEvent`
- plan renderer for GUI toolkit
- no duplicated behavior policy from TUI

## Progress snapshot (2026-02-12)

Completed:

- completion popup geometry and content planning moved into `xeno-editor`
- snippet choice popup geometry and content planning moved into `xeno-editor`
- statusline composition policy moved into `xeno-editor`
- utility/which-key planning moved into `xeno-editor`
- modal overlay pane/kind/rect plans exposed from `xeno-editor`
- frontend overlay reach-through removed (`overlay_interaction()` removed, overlay store access no longer needed in frontend crates)

Current focus:

- Phase 3 seam tightening: complete backend-neutral text/style boundaries
- `RenderLine`/`RenderSpan` are now backend-neutral with explicit frontend adaptation at TUI render sites

Next:

- add plan-builder snapshot/replay tests for cross-frontend behavior consistency
- bootstrap `xeno-editor-gui` crate against existing plan APIs once Phase 3 boundary is stable

## Guardrails (to avoid drift)

- rule: behavior logic belongs in core plan builders, not in frontend render code
- rule: frontend crates should consume plans, not editor internals
- snapshot tests for plan builders (state -> plan)
- replay/integration tests that verify identical plan/state progression under shared event scripts

## Non-goals

- creating a generic cross-toolkit draw trait in core
- forcing pixel-identical rendering between TUI and GUI

Consistency target is behavior and interaction semantics, not identical visual glyph output.

## Acceptance checks

Run for each boundary-sensitive milestone:

```bash
cargo check -p xeno-editor --all-targets
cargo check -p xeno-editor --no-default-features --all-targets
cargo check -p xeno-editor-tui
cargo check -p xeno-term
```

Plus targeted tests for touched subsystems (`focus`, `mouse`, `row`, overlay/panel tests).

## Initial execution order

1. completion/snippet/status plans
2. modal overlay plan and internal access removal
3. backend-neutral text/style completion
4. bootstrap GUI frontend on plan APIs
