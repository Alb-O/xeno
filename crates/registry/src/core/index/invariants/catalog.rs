//! Invariant catalog for [`crate::core::index::runtime::RuntimeRegistry`].
#![allow(dead_code)]

/// ID lookups must have exactly one winner per ID.
///
/// - Enforced in: [`crate::core::index::build::resolve_id_duplicates`], [`crate::core::index::runtime::RuntimeRegistry::register`]
/// - Tested by: [`crate::core::index::invariants::test_unambiguous_id_lookup`]
/// - Failure symptom: Build/startup panics or inconsistent `get_by_id` results.
pub(crate) const UNAMBIGUOUS_ID_LOOKUP_HAS_SINGLE_WINNER: () = ();

/// Runtime ID overrides must evict old def winners from key maps.
///
/// - Enforced in: [`crate::core::index::runtime::RuntimeRegistry::register`]
/// - Tested by: [`crate::core::index::invariants::test_id_override_eviction`]
/// - Failure symptom: Name/alias lookups return defs that no longer own the ID.
pub(crate) const EVICT_OLD_DEFINITION_ON_ID_OVERRIDE: () = ();

/// Effective definition iteration order must remain deterministic.
///
/// - Enforced in: [`crate::core::index::build::resolve_id_duplicates`]
/// - Tested by: [`crate::core::index::invariants::test_deterministic_iteration`]
/// - Failure symptom: `all()`/`iter()` ordering changes across runs or overrides.
pub(crate) const DETERMINISTIC_EFFECTIVE_ITERATION_ORDER: () = ();

/// Owned runtime definitions must remain alive while reachable from snapshots.
///
/// - Enforced in: [`crate::core::index::runtime::Snapshot`], [`crate::core::index::runtime::RegistryRef`]
/// - Tested by: [`crate::core::index::invariants::test_snapshot_liveness_across_swap`]
/// - Failure symptom: Use-after-free when readers hold snapshot-backed refs during writes.
pub(crate) const OWNED_DEFINITIONS_STAY_ALIVE_WHILE_REACHABLE: () = ();

/// Concurrent registrations must not lose updates.
///
/// - Enforced in: [`crate::core::index::runtime::RuntimeRegistry::register`] (CAS loop)
/// - Tested by: [`crate::core::index::invariants::test_no_lost_updates`]
/// - Failure symptom: Concurrent registrations silently dropped, missing definitions.
pub(crate) const NO_LOST_UPDATES_UNDER_CONTENTION: () = ();

/// Symbols must remain stable/resolvable across snapshot swaps.
///
/// - Enforced in: Interner prefix-copy in register()
/// - Tested by: [`crate::core::index::invariants::test_symbol_stability_across_swap`]
/// - Failure symptom: Held RegistryRefs return wrong strings or panic on resolve after swap.
pub(crate) const SYMBOL_STABILITY_ACROSS_SWAPS: () = ();
