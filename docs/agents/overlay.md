# Overlay System

## Purpose
- Owns: focus-stealing modal interactions (`OverlayManager`), passive contextual UI layers (`OverlayLayers`), and shared type-erased state (`OverlayStore`).
- Does not own: floating window rendering (owned by window subsystem), LSP request logic.
- Source of truth: `OverlaySystem` struct in `crates/editor/src/overlay/mod.rs`.

## Mental model
- Terms: Session (active modal interaction), Controller (behavior logic), Layer (passive UI), Spec (declarative UI layout), Capture (pre-preview state snapshot).
- Lifecycle in one sentence: A controller defines a UI spec, a host allocates resources for a session, and the system restores captured state on close.

## Module map
- `crates/editor/src/overlay/mod.rs` — Core system, traits (`OverlayController`, `OverlayLayer`), and routing.
- `crates/editor/src/overlay/host.rs` — `OverlayHost`: resource allocator and restorer.
- `crates/editor/src/overlay/session.rs` — `OverlaySession`: resource tracking and state capture.
- `crates/editor/src/overlay/spec.rs` — Declarative UI spec and `RectPolicy` geometry resolution.
- `crates/editor/src/overlay/controllers/` — Built-in implementations (search, palette, rename, info popup).
- `crates/editor/src/impls/interaction.rs` — Editor integration entry points.

## Key types
| Type | Meaning | Constraints | Constructed / mutated in |
|---|---|---|---|
| `OverlayUiSpec` | Declarative UI configuration | Static geometry resolve | Controller (`ui_spec`) |
| `OverlaySession` | Active session resources | MUST be torn down | `crates/editor/src/overlay/host.rs`::`setup_session` |
| `PreviewCapture` | Versioned state snapshot | Version-aware restore | `crates/editor/src/overlay/session.rs`::`capture_view` |
| `LayerEvent` | Payloaded UI events | Broadcast to all layers | `crates/editor/src/impls/interaction.rs`::`Editor::notify_overlay_event` |

## Invariants (hard rules)
1. MUST restore state ONLY if buffer version matches capture.
   - Enforced in: `crates/editor/src/overlay/session.rs`::`OverlaySession::restore_all`
   - Tested by: TODO (add regression: test_versioned_restore)
   - Failure symptom: User edits clobbered by preview restoration.
2. MUST NOT allow multiple active modal sessions.
   - Enforced in: `crates/editor/src/overlay/mod.rs`::`OverlayManager::open`
   - Tested by: TODO (add regression: test_exclusive_modal)
   - Failure symptom: Multiple focus-stealing prompts overlapping and fighting for keys.
3. MUST clamp all resolved window areas to screen bounds.
   - Enforced in: `crates/editor/src/overlay/spec.rs`::`RectPolicy::resolve_opt`
   - Tested by: `crates/editor/src/overlay/spec.rs`::`test_rect_policy_top_center_clamping`
   - Failure symptom: Windows rendering partially off-screen or zero-sized.
4. MUST clear LSP UI when a modal overlay opens.
   - Enforced in: `crates/editor/src/overlay/mod.rs`::`OverlayManager::open`
   - Tested by: TODO (add regression: test_modal_overlay_clears_lsp_menu)
   - Failure symptom: Completion menus appearing on top of modal prompts.

## Data flow
1. Trigger: Editor calls `interaction.open(controller)`.
2. Allocation: `OverlayHost` resolves spec, creates scratch buffers/windows, and focuses input.
3. Events: Editor emits `LayerEvent` (CursorMoved, etc.) via `notify_overlay_event`.
4. Update: Input changes in `session.input` call `controller.on_input_changed`.
5. Restoration: On cancel/blur, `session.restore_all` reverts previews (version-aware).
6. Teardown: `session.teardown` closes all windows and removes buffers.

## Lifecycle
- Open: `OverlayManager::open` calls `host.setup_session` then `controller.on_open`.
- Update: `OverlayManager::on_buffer_edited` filters for `session.input`.
- Commit: `OverlayManager::commit` runs `controller.on_commit` (async), then teardown.
- Cancel: `OverlayManager::close(Cancel)` runs `session.restore_all`, then teardown.
- Teardown: `OverlaySession::teardown` (idempotent resource cleanup).

## Concurrency & ordering
- Single-threaded UI: Most overlay operations run on the main UI thread.
- Async Commit: `on_commit` returns a future, allowing async operations (LSP rename) before cleanup.

## Failure modes & recovery
- Missing Anchor: `RectPolicy::Below` returns `None` if the target role is missing; host skips that window.
- Stale Restore: `restore_all` skips buffers with version mismatches to protect user edits.
- Focus Loss: `CloseReason::Blur` triggers automatic cancellation if `dismiss_on_blur` is set in spec.

## Recipes
### Add a new modal interaction
Steps:
- Create a struct implementing `OverlayController`.
- Implement `ui_spec` with `RectPolicy`.
- Wire entry point in `crates/editor/src/impls/interaction.rs`.
- Invariants: Use `session.preview_select` for safe buffer previews.

## Tests
- `crates/editor/src/overlay/spec.rs`::`test_rect_policy_top_center_clamping`
- `crates/editor/src/overlay/spec.rs`::`test_rect_policy_below_clamping`
- `crates/editor/src/overlay/mod.rs`::`overlay_store_get_or_default_is_stable_and_mutable`

## Glossary
- Session: A short-lived context for a modal interaction.
- Controller: The behavioral logic for an overlay.
- Layer: A passive, non-focusing UI component.
- Spec: The declarative description of an overlay's UI.
- Capture: A snapshot of buffer state taken before transient preview edits.
