//! Registry action effect interpreter over capability boundaries.
//! Anchor ID: XENO_ANCHOR_EFFECTS_BOUNDARY
//!
//! # Purpose
//!
//! * Interprets registry action outcomes (`ActionResult`/`ActionEffects`) into editor mutations through capability traits.
//! * Keeps registry semantics engine-agnostic by operating only on [`EditorContext`], not concrete `Editor`.
//! * Acts as the policy bridge between effect-oriented actions and editor effect sink/layer notifications.
//!
//! # Mental model
//!
//! * Actions produce data (`ActionEffects`) instead of mutating editor internals directly.
//! * This module is a capability-first interpreter: each effect variant maps to a narrow capability operation.
//! * Side effects are emitted through capability providers and then flushed by the editor effect sink.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`xeno_registry::actions::ActionEffects`] | Ordered effect list from action handlers | Must be applied in-order by interpreter | action handlers, `apply_effects` |
//! | [`xeno_registry::actions::editor_ctx::EditorContext`] | Capability façade for editor access | Must remain engine-agnostic and downcast-free | command/action execution paths |
//! | [`xeno_registry::actions::Effect`] | Effect variant union | Must map to specific apply path (`View`/`Edit`/`Ui`/`App`) | `apply_effects` |
//! | [`crate::effects::sink::EffectSink`] | Deferred side-effect queue | Must be the single downstream sink for visual/UI consequences | editor lifecycle flush paths |
//! | [`crate::capabilities::EditorCaps`] | Editor capability provider | Must be sole trait implementation boundary for registry capabilities | `Editor::caps` |
//!
//! # Invariants
//!
//! * Must not use RTTI or engine-specific downcasting to reach concrete editor internals.
//! * Must route all side effects through capability providers and effect sink flush paths.
//! * Must preserve effect ordering from `ActionEffects`.
//! * Must handle unknown effect variants as debug assertions plus safe no-op traces.
//!
//! # Data flow
//!
//! 1. Invocation/command path resolves an action result.
//! 2. `dispatch_result` enters this interpreter with `EditorContext`.
//! 3. `apply_effects` iterates ordered effects and delegates to variant handlers.
//! 4. Capability methods enqueue downstream UI/layer/overlay side effects.
//! 5. Editor lifecycle later drains the effect sink via `flush_effects`.
//!
//! # Lifecycle
//!
//! * Construct `EditorCaps`, wrap in `EditorContext`, and call `dispatch_result`.
//! * Execute `apply_effects` synchronously in invocation path.
//! * Runtime/lifecycle flush applies deferred sink consequences.
//!
//! # Concurrency & ordering
//!
//! * Interpreter runs synchronously on the editor thread.
//! * Ordering is strictly in effect-list order.
//! * Re-entrant side effects are deferred by flush-depth logic in effect sink layer.
//!
//! # Failure modes & recovery
//!
//! * Missing optional capability: effect branch becomes no-op with trace logging.
//! * Unsupported effect variant: debug assertion plus no-op in release.
//! * Overlay request validation failure: converted to command error at sink boundary.
//!
//! # Recipes
//!
//! * Add a new effect variant:
//!   1. Extend registry effect enum.
//!   2. Add interpreter arm in `apply_*_effect`.
//!   3. Add invariant/test proving ordering and sink routing.
//! * Add a new capability-backed operation:
//!   1. Add capability trait surface.
//!   2. Implement in `EditorCaps`.
//!   3. Route interpreter arm through that capability.
//!

mod core;

pub use core::apply_effects;
pub(crate) use core::register_result_handlers;

pub use xeno_registry::actions::editor_ctx::*;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
