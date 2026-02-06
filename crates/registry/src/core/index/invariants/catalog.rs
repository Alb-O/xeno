//! Invariant catalog for [`crate::core::index::runtime::RuntimeRegistry`].
#![allow(dead_code)]

/// ID lookups must have exactly one winner per ID.
///
/// - Enforced in: [`crate::core::index::insert::insert_typed_key`], [`crate::core::index::insert::insert_id_key_runtime`]
/// - Tested by: [`crate::core::index::invariants::test_unambiguous_id_lookup`]
/// - Failure symptom: Build/startup panics or inconsistent `get_by_id` results.
pub(crate) const UNAMBIGUOUS_ID_LOOKUP_HAS_SINGLE_WINNER: () = ();

/// Runtime ID overrides must evict old def winners from key maps.
///
/// - Enforced in: [`crate::core::index::insert::insert_id_key_runtime`]
/// - Tested by: [`crate::core::index::invariants::test_id_override_eviction`]
/// - Failure symptom: Name/alias lookups return defs that no longer own the ID.
pub(crate) const EVICT_OLD_DEFINITION_ON_ID_OVERRIDE: () = ();

/// Effective definition iteration order must remain deterministic.
///
/// - Enforced in: [`crate::core::index::runtime::RuntimeRegistry::try_register_many_internal`]
/// - Tested by: [`crate::core::index::invariants::test_deterministic_iteration`]
/// - Failure symptom: `all()`/`iter()` ordering changes across runs or overrides.
pub(crate) const DETERMINISTIC_EFFECTIVE_ITERATION_ORDER: () = ();

/// Owned runtime definitions must remain alive while reachable from snapshots.
///
/// - Enforced in: [`crate::core::index::runtime::Snapshot`], [`crate::core::index::runtime::RegistryRef`]
/// - Tested by: [`crate::core::index::invariants::test_snapshot_liveness_across_swap`]
/// - Failure symptom: Use-after-free when readers hold snapshot-backed refs during writes.
pub(crate) const OWNED_DEFINITIONS_STAY_ALIVE_WHILE_REACHABLE: () = ();
