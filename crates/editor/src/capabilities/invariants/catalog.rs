//! Catalog of architectural invariants for the capability provider.

#![allow(dead_code)]

/// - The Delegator Rule: The [`crate::impls::Editor`] struct must not implement any `xeno_registry::*Access`
///   traits. All registry-facing capabilities must be implemented on [`crate::capabilities::provider::EditorCaps`].
///   - Enforced in: [`crate::capabilities::provider::EditorCaps`] (via delegator pattern)
///   - Tested by: [`crate::capabilities::invariants::test_delegator_rule`]
///   - Failure symptom: Circular dependencies or accidental leakage of engine-specific methods into the action registry.
pub(crate) const DELEGATOR_RULE: () = ();

/// - Mutation Side-Effect Invariant: Capability methods on `EditorCaps` that mutate editor state
///   must enqueue corresponding events into the `EffectSink`.
///   - Enforced in: [`crate::capabilities::provider::EditorCaps`] (via domain-specific implementations)
///   - Tested by: [`crate::capabilities::invariants::test_mutation_side_effect_invariant`]
///   - Failure symptom: UI layers (overlays, status bars) failing to update after an action executes.
pub(crate) const MUTATION_SIDE_EFFECT_INVARIANT: () = ();
