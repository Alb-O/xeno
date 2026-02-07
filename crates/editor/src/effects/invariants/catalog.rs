//! Catalog of architectural invariants for the side-effect system.

#![allow(dead_code)]

/// - Single Path Side-Effects: All UI-visible consequences of editor mutations
///   (overlays, notifications, redraws) must be routed through the `EffectSink` and
///   dispatched via `flush_effects`.
///   - Enforced in: [`crate::impls::Editor::flush_effects`]
///   - Tested by: [`crate::effects::invariants::test_single_path_side_effects`]
///   - Failure symptom: Inconsistent UI state or dropped event notifications.
pub(crate) const SINGLE_PATH_SIDE_EFFECTS: () = ();
