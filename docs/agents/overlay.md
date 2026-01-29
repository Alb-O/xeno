# Xeno overlay system (controllers + layers)

This guide describes the unified overlay architecture in the editor UI stack: modal interactions implemented via controllers (search/palette/rename) and passive/inline UI implemented as layers (info popups, completion menus, signature help, etc). The system also includes a type-erased store for shared overlay state.

Key files (read these first):
- Core system, traits, and routing:
  - crates/editor/src/overlay/mod.rs
- Modal session resource management:
  - crates/editor/src/overlay/host.rs
  - crates/editor/src/overlay/session.rs
- Declarative UI spec + geometry policies:
  - crates/editor/src/overlay/spec.rs
- Built-in controllers and layers:
  - crates/editor/src/overlay/controllers/*
- Editor integration entrypoints:
  - crates/editor/src/impls/interaction.rs

Scope:
- Modal prompts: SearchOverlay, RenameOverlay, CommandPaletteOverlay
- Passive layers: OverlayLayer trait, OverlayLayers stack, InfoPopupLayer example
- Shared state: OverlayStore

Non-goals:
- Window-manager internals (floating window rendering is owned by the window subsystem)
- LSP completion/signature help logic (this guide covers how they should attach to overlays)

---

## System overview

The overlay system is split along the focus boundary:

Modal interactions (focus-stealing)
- Own a dedicated scratch input buffer.
- Own one or more floating windows.
- Take focus; editor key dispatch is short-circuited to the interaction first.
- Example: command palette, rename prompt, search prompt.

Passive layers (non-focusing / inline)
- Do not take focus and do not allocate scratch input buffers.
- Render contextual UI on top of the editor (tooltips, completion lists) and/or observe events to dismiss or update themselves.
- May optionally intercept a small key subset when visible (e.g. completion accept/next/prev) but do not own focus.
- Example: info popup layer (dismiss on cursor move), completion menu, signature help.

Both kinds are orchestrated by OverlaySystem:
- interaction: OverlayManager (single active modal session)
- layers: OverlayLayers (stack of visible contextual layers)
- store: OverlayStore (type-erased shared state for overlays)

---

## Mental model

OverlaySystem is a UI stack with two lanes:

lane 1: modal session (at most one)
- OverlayManager holds ActiveOverlay { session, controller }
- ActiveOverlay is exclusive; no stacking unless you explicitly extend OverlayManager later.

lane 2: layer stack (0..N)
- OverlayLayers is an ordered Vec of OverlayLayer objects.
- Render order is insertion order (first added renders first; later layers draw on top).
- Key routing is reverse order (topmost layer gets first chance).

Key routing priority (typical call pattern)
1) If a modal interaction is active:
   - OverlayManager::handle_key(...) runs first.
   - If consumed, stop.
2) Else / or after interaction declines:
   - OverlayLayers::handle_key(...) runs from topmost visible layer down.
3) If nobody consumes:
   - normal editor keymap executes.

Event routing (cursor moved, mode changed, buffer edited, focus changed)
- The editor emits LayerEvent values into OverlayLayers::notify_event(...)
- Layers decide whether to update/dismiss based on event and its payload.

State sharing
- OverlayStore is a global type-erased map used by layers and the editor to stash overlay-related state (completion model, signature help snapshot, info popup queue, etc).
- The store exists to avoid adding N bespoke fields to EditorState.

---

## Core components

### OverlaySystem

Defined in crates/editor/src/overlay/mod.rs.

Fields:
- overlay_system.interaction: OverlayManager
- overlay_system.layers: OverlayLayers
- overlay_system.store: OverlayStore

Constructor:
- OverlaySystem::new() initializes OverlayLayers and registers built-in layers.
- Current default layer set: InfoPopupLayer (controllers::InfoPopupLayer).

Guiding rule:
- OverlaySystem is the single root for UI overlays. If something is an overlay, its persistent state belongs in store or in the layer/controller object, and its rendering/lifecycle belongs in interaction or layers.

### OverlayManager (modal interaction lane)

Defined in crates/editor/src/overlay/mod.rs.

Fields:
- active: Option<ActiveOverlay>

ActiveOverlay:
- session: OverlaySession (resource IDs + restoration + preview capture + status)
- controller: Box<dyn OverlayController> (behavior)

Invariants:
- At most one active modal session.
- OverlayHost owns allocation and destruction; controllers never directly create/destroy scratch buffers/windows.
- Focus, mode, and transient preview edits are restored by the host on close (unless CloseReason::Commit).

Primary APIs:
- open(ed, controller) -> bool
- close(ed, reason)
- commit(ed).await
- handle_key(ed, key) -> bool
- on_buffer_edited(ed, view_id)
- on_viewport_changed(ed) (currently TODO reflow)

### OverlayController (modal behavior)

Trait defined in crates/editor/src/overlay/mod.rs.

Responsibilities:
- Describe the UI: ui_spec()
- Implement session behavior:
  - on_open(): initialize input buffer content, capture target state, etc
  - on_input_changed(): incremental preview, filtering, status messages
  - on_key(): optional key interception beyond host defaults
  - on_commit(): async final action (LSP rename, command dispatch, etc)
  - on_close(): controller-specific cleanup (rare; session and resources are host-managed)

Important constraint:
- OverlayController is behavioral only. It must treat OverlayHost and OverlaySession as the only way to read/modify resources. Do not reach into window manager state to create/remove windows.

### OverlayHost (resource allocator + restorer)

Defined in crates/editor/src/overlay/host.rs.

Setup path: setup_session(ed, controller) -> Option<OverlaySession>
1) Reads controller.ui_spec(ed).
2) Resolves screen rect from viewport.
3) Captures origin state:
   - origin_focus: FocusTarget
   - origin_view: ViewId (focused view at open)
   - origin_mode: Mode (mode of focused buffer at open)
4) Allocates scratch buffers and windows:
   - Primary input buffer/window is created from OverlayUiSpec rect/style/gutter.
   - Primary input window is always sticky and dismiss-on-blur.
   - Auxiliary windows are created from OverlayUiSpec.windows (WindowSpec list).
   - Buffer-local options from WindowSpec.buffer_options are applied via set_by_kdl.
5) Focuses primary input and forces its buffer mode to Insert.
6) Returns OverlaySession populated with all resource IDs + origin state.

Cleanup path: cleanup_session(ed, controller, session, reason)
1) controller.on_close(ed, &mut session, reason)
2) If reason != Commit:
   - session.restore_all(ed) restores captured cursor/selection for any preview-mutated views if buffer version matches.
3) session.teardown(ed) closes floating windows and removes scratch buffers.
4) Restores:
   - ed.state.focus = session.origin_focus
   - origin_view buffer mode = session.origin_mode
5) Marks frame dirty (needs_redraw).

CloseReason:
- Cancel: explicit cancel (Esc)
- Commit: accept (Enter / commit action)
- Blur: lost focus (dismiss_on_blur)
- Forced: programmatic shutdown

Host default dismissal (in OverlayManager::handle_key):
- If controller.on_key() does not consume and key == Escape: close(Cancel).

### OverlaySession (session-scoped state + helpers)

Defined in crates/editor/src/overlay/session.rs.

Resources:
- windows: Vec<WindowId>
- buffers: Vec<ViewId>
- input: ViewId (primary input buffer)

Restoration:
- origin_focus: FocusTarget
- origin_mode: Mode
- origin_view: ViewId

Transient preview capture:
- capture: PreviewCapture { per_view: HashMap<ViewId, (u64, CharIdx, Selection)> }
- Stores buffer version to prevent clobbering user edits during restoration.

Status:
- status: OverlayStatus { message: Option<(StatusKind, String)> }
- StatusKind = Info | Warn | Error

Session APIs used by controllers:
- input_text(ed) -> String
- capture_view(ed, view)
- preview_select(ed, view, range) (captures then sets cursor+selection atomically)
- restore_all(ed) (non-destructive; version-aware)
- clear_capture()
- teardown(ed) (resource cleanup)
- set_status(kind, msg)
- clear_status()

Invariants and usage rules:
- Any controller that mutates a non-input view for preview must call capture_view() first (or use preview_select()).
- restore_all() is intended for cancel paths and “clear input restores origin” behavior.
- teardown() must be called to ensure resource cleanup.

### OverlayLayers (passive/inline lane)

Defined in crates/editor/src/overlay/mod.rs.

Structure:
- layers: Vec<Box<dyn OverlayLayer>>

Semantics:
- Render order: insertion order.
  - Later layers draw over earlier layers.
- Key routing: reverse order among visible layers (topmost first).

APIs:
- add(layer)
- handle_key(ed, key) -> bool
- notify_event(ed, event)
- render(ed, frame)

Screen rect:
- OverlayLayers::render resolves the screen rect from viewport width/height and passes it to each layer layout().

LayerEvent:
- CursorMoved { view: ViewId }
- ModeChanged { view: ViewId, mode: Mode }
- BufferEdited(ViewId)
- FocusChanged { from: FocusTarget, to: FocusTarget }

The editor is responsible for emitting LayerEvent(s) via Editor::notify_overlay_event().

### OverlayLayer (passive/inline behavior)

Trait defined in crates/editor/src/overlay/mod.rs.

Responsibilities:
- is_visible(ed) -> bool (cheap; called often)
- layout(ed, screen) -> Option<Rect> (compute placement; handle clamping)
- render(ed, frame, area) (draw)
- on_key(ed, key) -> bool (optional; only called when visible)
- on_event(ed, event) (optional; update/dismiss on events)

Layer design rule:
- Layers do not own focus.
- Layers should not create scratch buffers or windows.
- Layers that need persistent state should keep it in OverlayStore or inside their layer object (if per-layer state is sufficient).

Example: InfoPopupLayer
- is_visible uses OverlayStore state (InfoPopupStore) to decide visibility.
- on_event closes popups on relevant CursorMoved/ModeChanged/FocusChanged.
- Note: Completion and Signature Help currently use direct render hooks in LspSystem rather than the OverlayLayer trait, but they use OverlayStore for state.

---

## Lifecycle and lifecycle events

### Modal session lifecycle

Open
- Call site constructs controller and calls OverlayManager::open(ed, controller).
  - Typical entrypoints live in crates/editor/src/impls/interaction.rs:
    - Editor::open_search(reverse)
    - Editor::open_command_palette()
    - Editor::open_rename()
- OverlayManager::open:
  - Rejects if active already exists.
  - OverlayHost::setup_session allocates resources + focuses input.
  - controller.on_open runs post-allocation.

Update (incremental)
- The editor must call OverlayManager::on_buffer_edited(ed, view_id) when the input buffer changes.
- OverlayManager filters by session.input:
  - if view_id != session.input: ignore
  - else: session.input_text(ed) and controller.on_input_changed(...)

Commit/cancel
- commit:
  - OverlayManager::commit(ed).await
  - takes ActiveOverlay out of manager (no re-entrancy)
  - calls controller.on_commit(ed, &mut session).await
  - calls OverlayHost::cleanup_session(..., Commit)
- cancel / blur / forced:
  - OverlayManager::close(ed, reason)
  - calls OverlayHost::cleanup_session(..., reason)

Host cleanup invariants:
- reason != Commit => session.restore_all(ed) is invoked before teardown.
- focus and mode are restored after resources are closed/removed.

### Layer lifecycle

Layers are long-lived objects registered in OverlaySystem::new() (or later via OverlayLayers::add()).

Visibility
- A layer is eligible to run layout/render/key interception only if is_visible(ed) returns true.
- is_visible must be cheap and side-effect free.

Layout/render
- OverlayLayers::render iterates stack order and for each visible layer:
  - area = layer.layout(ed, screen)
  - if Some(area): layer.render(ed, frame, area)

Events
- The editor sends LayerEvent notifications via Editor::notify_overlay_event(event).
- Layers use these to update their internal state in OverlayStore or dismiss themselves.

Typical event policy patterns:
- CursorMoved: dismiss tooltips, invalidate completion anchor, recompute layout, etc
- ModeChanged: dismiss insert-only overlays (completion/signature) on leaving Insert
- BufferEdited(view): update completion filter for that buffer, invalidate signature help, etc

Key interception
- OverlayLayers::handle_key routes keys to visible layers in reverse order.
- Use this for inline-interactive overlays (completion accept/next/prev) without stealing focus.

---

## Declarative UI: OverlayUiSpec and RectPolicy

OverlayUiSpec is the declarative configuration a controller returns from ui_spec().

Defined in crates/editor/src/overlay/spec.rs:
- title: Option<String> (currently used by prompt_style; window manager draws it)
- gutter: GutterSelector (primary window gutter)
- rect: RectPolicy (primary window rect policy)
- style: FloatingStyle (primary window border/padding/title/shadow)
- windows: Vec<WindowSpec> (aux windows)

WindowSpec:
- role: WindowRole (Input | List | Preview | Custom(&'static str))
- rect: RectPolicy (can be relative via Below)
- style: FloatingStyle
- buffer_options: HashMap<String, OptionValue> (applied via set_by_kdl)
- dismiss_on_blur: bool
- sticky: bool
- gutter: GutterSelector

RectPolicy:
- TopCenter { width_percent, max_width, min_width, y_frac, height }
- Below(WindowRole, offset_y, height)

Resolution model (in OverlayHost::setup_session):
- roles: HashMap<WindowRole, Rect> is built as windows resolve.
- Primary input rect is resolved first and inserted as role Input.
- Auxiliary windows resolve their rect against roles; Below uses roles.get(role) and positions directly below it.
- All rects are clamped to screen bounds during resolution (resolve_opt).

Rules:
- RectPolicy::Below requires the referenced role to already be resolved. If the role is missing, resolve_opt returns None.
- Keep rect policies simple and deterministic. Geometry belongs in spec, not in controller code.

Viewport changes
- OverlayManager::on_viewport_changed is currently TODO.
- Once implemented, the host should recompute rects from the original OverlayUiSpec and update floating window rects in-place. This requires session to retain the spec or the host to be able to recompute via controller.ui_spec(ed) + stable role ordering.

---

## Shared state: OverlayStore

OverlayStore is a passive type-erased map used to stash overlay state without adding dedicated EditorState fields.

Defined in crates/editor/src/overlay/mod.rs:
- inner: HashMap<TypeId, Box<dyn Any + Send + Sync>>

API:
- get<T>() -> Option<&T>
- get_mut<T>() -> Option<&mut T>
- get_or_default<T: Default>() -> &mut T
- insert<T>(val)

Rules:
- Types stored must be Any + Send + Sync and effectively 'static.
- OverlayStore is not versioned and has no lifetime tracking. Store only editor-owned state (not borrowed pointers into transient data).
- Use OverlayStore for cross-cutting overlay state (LSP completion model, info popup queues, cached docs, etc).
- Do not use OverlayStore for modal interaction session state; that belongs in the controller object or session.

Example usage (InfoPopupLayer):
- is_visible checks ed.overlays().get::<InfoPopupStore>() and tests emptiness.
- on_event clears state by calling ed.close_all_info_popups().

---

## Task-first recipes

### Add a new modal interaction (controller)

Goal: implement a new focus-stealing interaction (e.g. fuzzy file picker, registry picker).

Steps:
1) Create controller type in crates/editor/src/overlay/controllers/<name>.rs implementing OverlayController.
2) Implement ui_spec():
   - Choose prompt_style(...) or a custom FloatingStyle.
   - Use RectPolicy::TopCenter for input; add WindowSpec entries for list/preview if needed.
3) Implement on_open():
   - Prefill input buffer if needed (mutate session.input buffer).
   - Capture any target view state you will preview-mutate: session.capture_view(ed, target).
4) Implement on_input_changed():
   - Read current input (provided as text), update internal filter state, update auxiliary buffers or status.
   - For preview mutations on editor buffers, use session.preview_select and session.restore_all.
5) Implement on_commit():
   - Read input via session.input_text(ed).
   - Perform final action (sync or async).
6) Wire entrypoint in editor integration (typically crates/editor/src/impls/interaction.rs):
   - construct controller, call overlay_system.interaction.open(self, Box::new(controller))

Verify:
- rg -n "impl OverlayController" crates/editor/src/overlay/controllers
- cargo test / run UI smoke test
- Ensure Cancel restores focus/mode and any preview state (if versions match).

### Add a new passive layer

Goal: implement a contextual UI element (e.g. completion popup, signature help).

Steps:
1) Create layer type implementing OverlayLayer.
2) Decide state location:
   - If the layer needs persistent model data shared with editor/LSP, define a store type and use OverlayStore::get_or_default to access it.
   - If state is strictly internal, keep it in the layer object.
3) Implement is_visible(ed) based on store state and editor mode/buffer focus.
4) Implement layout(ed, screen) to return a rect:
   - anchor to cursor (requires editor helper; not shown here)
   - or use fixed placement (TopRight, Bottom, etc) if suitable
   - handle screen clamping
5) Implement render(ed, frame, area).
6) Register the layer in OverlaySystem::new() via layers.add(Box::new(MyLayer)).

Events:
- Ensure the editor emits LayerEvent values via notify_overlay_event().

Key interception:
- If the layer is inline-interactive (completion), implement on_key and only consume an explicit allowlist of keys.

### Store a new overlay state type

Goal: share overlay state across the editor and layers without adding a core state field.

Steps:
1) Define a concrete state struct, e.g. CompletionState { ... }.
2) Write access:
   - let st = ed.overlays_mut().get_or_default::<CompletionState>();
3) Read access:
   - if let Some(st) = ed.overlays().get::<CompletionState>() { ... }

Avoid:
- Storing references into documents/buffers that can be invalidated.
- Using OverlayStore for per-session transient state (put that in controller/session).

---

## Grep/verify cheatsheet

Modal interaction core:
- rg -n "struct OverlaySystem|struct OverlayManager|trait OverlayController|struct OverlayHost" crates/editor/src/overlay

Layer stack:
- rg -n "trait OverlayLayer|struct OverlayLayers|LayerEvent" crates/editor/src/overlay

UI spec / geometry:
- rg -n "struct OverlayUiSpec|enum RectPolicy|WindowSpec|WindowRole" crates/editor/src/overlay/spec.rs

Controllers and layers:
- rg -n "impl OverlayController|impl OverlayLayer" crates/editor/src/overlay/controllers

Editor entrypoints:
- rg -n "open_search\(|open_command_palette\(|open_rename\(|interaction_" crates/editor/src/impls/interaction.rs

---

## Minimal test checklist (UI stack correctness)

Modal interactions
- Opening a controller allocates exactly the expected buffers/windows and focuses session.input.
- Cancel (Esc) restores:
  - focus target (origin_focus)
  - origin buffer mode (origin_mode)
  - preview-mutated views (cursor/selection restore_all if version matches)
- Commit does not restore preview captures unless controller does it explicitly.
- Session cleanup always removes scratch buffers and closes all floating windows (teardown).
- on_buffer_edited only triggers on session.input changes; editing other buffers does not call controller.on_input_changed.

RectPolicy
- TopCenter respects min_width/max_width and y_frac placement, with clamping.
- Below positions relative to a previously resolved role and uses correct width, with clamping.

Layers
- Render order matches insertion order; key routing is reverse order.
- notify_overlay_event broadcasts CursorMoved/ModeChanged/BufferEdited/FocusChanged and layers can dismiss/update.
- InfoPopupLayer dismisses popups on CursorMoved, ModeChanged, and FocusChanged.

Store
- get_or_default inserts exactly once per type and returns stable mutable reference.
- Downcasting failures are impossible under correct usage; no TypeId collisions.

Known TODOs / follow-ups
- Implement OverlayManager::on_viewport_changed + host reflow.
- Migrate InfoPopupLayer from event-only to true layer rendering or host-managed passive window type (remove the “layout returns None” special case).
- Gate LSP UI against modal overlays in LspSystem.
