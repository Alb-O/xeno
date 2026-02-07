//! Catalog of architectural invariants for the effect interpreter.

#![allow(dead_code)]

/// - The Honesty Rule: The interpreter must not use RTTI, `Any`, or engine-specific
///   downcasting to access `Editor` methods.
///   - Enforced in: [`crate::editor_ctx::apply_effects`]
///   - Tested by: [`crate::editor_ctx::invariants::test_honesty_rule`]
///   - Failure symptom: Compilation error or boundary breach that couples registry policy to engine implementation.
pub(crate) const HONESTY_RULE: () = ();

/// - Single Path Side-Effects: All UI consequences of an action (redraws, overlay
///   notifications) must originate from the capability providers enqueuing into the
///   `EffectSink`. The interpreter must not emit these events directly.
///   - Enforced in: [`crate::editor_ctx::apply_effects`]
///   - Tested by: [`crate::editor_ctx::invariants::test_single_path_side_effects`]
///   - Failure symptom: Duplicate notifications or missed UI updates during re-entrant actions.
pub(crate) const SINGLE_PATH_SIDE_EFFECTS: () = ();
