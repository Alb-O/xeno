//! Machine-checkable invariant catalog and proof entrypoints for registry indexing.
#![allow(dead_code)]

pub(crate) mod catalog;

#[allow(unused_imports)]
pub(crate) use catalog::{
	DETERMINISTIC_EFFECTIVE_ITERATION_ORDER, EVICT_OLD_DEFINITION_ON_ID_OVERRIDE,
	OWNED_DEFINITIONS_STAY_ALIVE_WHILE_REACHABLE, UNAMBIGUOUS_ID_LOOKUP_HAS_SINGLE_WINNER,
};

#[cfg(doc)]
pub(crate) fn test_snapshot_liveness_across_swap() {}

#[cfg(doc)]
pub(crate) fn test_unambiguous_id_lookup() {}

#[cfg(doc)]
pub(crate) fn test_id_override_eviction() {}

#[cfg(doc)]
pub(crate) fn test_deterministic_iteration() {}

#[cfg(test)]
mod proofs;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use proofs::{
	test_deterministic_iteration, test_id_override_eviction, test_snapshot_liveness_across_swap,
	test_unambiguous_id_lookup,
};
