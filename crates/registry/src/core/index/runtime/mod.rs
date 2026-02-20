//! Immutable runtime registry container.
//! Anchor ID: XENO_ANCHOR_REGISTRY_RUNTIME
//!
//! # Purpose
//!
//! Provide lock-free reads on top of immutable snapshots built at bootstrap.
//!
//! # Mental model
//!
//! * Readers pin an `Arc<Snapshot<...>>` and resolve lookups against that immutable view.
//! * There are no runtime writers. Publication happens once during bootstrap.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`crate::core::index::runtime::RuntimeRegistry`] | Immutable runtime registry wrapper | Must only expose read/query APIs | [`crate::core::index::runtime::RuntimeRegistry::new`] |
//! | [`crate::core::index::snapshot::Snapshot`] | Immutable published state | Must remain immutable after publish | [`crate::core::index::snapshot::Snapshot::from_builtins`] |
//! | [`crate::core::index::snapshot::RegistryRef`] | Snapshot-pinned entry handle | Must keep source snapshot alive | [`crate::core::index::runtime::RuntimeRegistry::get`] |
//!
//! # Invariants
//!
//! * Lookup stage precedence must be preserved: ID (`by_id`) then name (`by_name`) then key (`by_key`).
//!
//! # Data flow
//!
//! 1. Read path: `get*` loads current snapshot and resolves symbols through staged maps.
//!
//! # Lifecycle
//!
//! 1. Startup: `RuntimeRegistry::new` creates a snapshot from builtins.
//! 2. Steady state: readers use lock-free snapshot loads.
//!
//! # Concurrency & ordering
//!
//! * Readers are wait-free (`Arc` clone + immutable data reads).
//! * Ordering is deterministic through the build-time precedence contract.
//!
//! # Failure modes & recovery
//!
//! * Stale refs remain valid because they pin their originating snapshot.
//!
//! # Recipes
//!
//! ## Lookup with stable lifetime
//!
//! * Call `get` / `get_sym` / `get_by_id`.
//! * Keep the returned `RegistryRef` as long as data from that snapshot is needed.

use super::snapshot::{RegistryRef, Snapshot, SnapshotGuard};
use super::types::RegistryIndex;
use crate::core::{DenseId, RegistryEntry, Symbol};

mod state;

pub use state::{RuntimeEntry, RuntimeRegistry};

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod invariants;
