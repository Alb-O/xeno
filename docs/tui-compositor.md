# TUI Compositor and Overlay Surfaces

This note captures the current user-facing UI composition model after the surface migration.

## Render pipeline

Frame rendering is centralized in `crates/editor/src/ui/compositor.rs`.

The compositor builds a `UiScene` with stable z-order and dispatches each surface operation in order.

Current layer order:

1. `Background`
2. `Document`
3. `InfoPopups` (conditional)
4. `Panels`
5. `CompletionPopup` (conditional)
6. `OverlayLayers` (event-driven passive layer stack)
7. `ModalOverlays` (conditional)
8. `StatusLine`
9. `Notifications`
10. `WhichKeyHud` (conditional)

The surface model and hit-test primitives live in `crates/editor/src/ui/scene.rs`.

## Modal overlays

Modal interactions are buffer/pane based.

- Session resources are modeled in `crates/editor/src/overlay/session.rs` as `OverlayPane` entries plus scratch buffers.
- Allocation and reflow are handled in `crates/editor/src/overlay/host.rs`.
- Geometry for pane content area is shared through `crates/editor/src/overlay/geom.rs::pane_inner_rect`.
- Rendering happens in `crates/editor/src/ui/layers/modal_overlays.rs`.
- Resize updates call `OverlayManager::on_viewport_changed` and reflow active panes.

Reflow invariants:

- Unresolved input pane forces modal close.
- Unresolved auxiliary panes are zeroed to avoid stale geometry reuse.

## Info popups

Info popups are rendered as surface layers.

- Data and lifecycle: `crates/editor/src/info_popup/mod.rs`
- Rendering: `crates/editor/src/ui/layers/info_popups.rs`
- Overlay layer `InfoPopupLayer` remains registered for event-driven dismissal only (`crates/editor/src/overlay/controllers/info_popup.rs`).

## Async overlay outcomes

Async modal outcomes (for example rename results) travel through the editor message bus.

- Message type: `crates/editor/src/msg/overlay.rs`
- Integration: `crates/editor/src/msg/mod.rs`
- Deferred workspace edit application queue: `crates/editor/src/types/frame.rs`
- Runtime apply point: `crates/editor/src/runtime/mod.rs`
- Redraw signaling: overlay messages return `Dirty::REDRAW`

For rename, `on_commit` spawns async work and emits overlay messages instead of blocking modal close.

The compositor no longer includes any windowed overlay path; user-facing rendering is surface-based.
