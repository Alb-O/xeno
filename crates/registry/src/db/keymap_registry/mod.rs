//! Layered keymap compilation and runtime lookup.
//!
//! # Purpose
//!
//! * Compile keymap sources (action defaults, preset bindings, runtime bindings, user overrides)
//!   into an immutable runtime snapshot.
//! * Keep source collection, precedence policy, compile diagnostics, and runtime lookup as
//!   separate layers with explicit interfaces.
//!
//! # Mental model
//!
//! * `sources` gathers typed `SpecBinding` candidates and prefix metadata.
//! * `compiler` resolves targets, applies precedence per `(mode, sequence)` slot, and emits
//!   a `CompiledKeymap` artifact with diagnostics.
//! * `snapshot` materializes trie matchers from compiled slots for fast lookup and continuation queries.
//! * `runtime` caches a catalog-scoped immutable snapshot.
//!
//! # Key types
//!
//! | Type | Role |
//! |---|---|
//! | `KeymapSpec` | Collected source candidates before resolution. |
//! | `CompiledKeymap` | Compile artifact with resolved slots and diagnostics. |
//! | `KeymapSnapshot` | Immutable runtime lookup index used by input dispatch. |
//! | `KeymapSnapshotCache` | Snapshot cache keyed by immutable catalog version. |
//!
//! # Invariants
//!
//! * Must resolve one deterministic winner per `(mode, sequence)` slot.
//! * Must apply precedence in source order: override > preset > runtime-action > action-default.
//! * Must preserve unbind semantics (`None` override removes inherited bindings).
//! * Must expose compile diagnostics without aborting snapshot construction.
//!
//! # Data flow
//!
//! * Gather source bindings in `sources::collect_keymap_spec`.
//! * Resolve and prioritize in `KeymapCompiler::compile`.
//! * Convert compile artifact into runtime `KeymapSnapshot`.
//! * Serve lookup calls through `KeymapSnapshotCache` and snapshot APIs.
//!
//! # Lifecycle
//!
//! * Build snapshot once from immutable action catalog data.
//! * Reuse the same snapshot for the catalog lifetime.
//!
//! # Concurrency & ordering
//!
//! * Snapshot reads are immutable and lock-free.
//! * Compile ordering is deterministic through explicit precedence policy and sorted slot materialization.
//!
//! # Failure modes & recovery
//!
//! * Invalid key sequences and unknown action targets are reported as build problems.
//! * Invalid candidates are skipped, while remaining candidates still produce a usable snapshot.
//! * Stale snapshots remain valid until dropped by all readers.
//!
//! # Recipes
//!
//! * Add a new keymap source:
//!   1. Add collector logic under `sources/`.
//!   2. Emit `SpecBinding` candidates with source rank and diagnostics.
//!   3. Add precedence and compiler tests for the new source.
//! * Change precedence behavior:
//!   1. Update `precedence::compare_candidates`.
//!   2. Update invariant tests documenting winner rules.
//!   3. Re-run registry and input keymap tests.

mod compiler;
mod diagnostics;
mod precedence;
mod runtime;
mod snapshot;
mod sources;
mod spec;

pub use diagnostics::KeymapBuildProblem;
pub use runtime::{KeymapSnapshotCache, get_keymap_snapshot};
pub use snapshot::{CompiledBinding, CompiledBindingTarget, KeymapSnapshot, LookupOutcome};
pub use xeno_keymap_core::ContinuationKind;

#[cfg(test)]
mod invariants;
#[cfg(test)]
mod tests;
