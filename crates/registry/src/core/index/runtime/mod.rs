//! Runtime registry container with atomic publication.
//! Anchor ID: XENO_ANCHOR_REGISTRY_RUNTIME
//!
//! # Purpose
//!
//! Provide lock-free reads and linearizable runtime registration on top of immutable
//! snapshots.
//!
//! # Mental model
//!
//! * Readers pin an `Arc<Snapshot<...>>` and resolve lookups against that immutable view.
//! * Writers build a replacement snapshot and publish it with CAS.
//! * Failed CAS means "someone else won first"; writer retries from the latest snapshot.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`crate::core::index::runtime::RuntimeRegistry`] | Atomic runtime registry wrapper | Must publish via CAS to avoid lost updates | [`crate::core::index::runtime::RuntimeRegistry::register_internal`] |
//! | [`crate::core::index::snapshot::Snapshot`] | Immutable published state | Must remain immutable after publish | [`crate::core::index::snapshot::Snapshot::from_builtins`] |
//! | [`crate::core::index::snapshot::RegistryRef`] | Snapshot-pinned entry handle | Must keep source snapshot alive | [`crate::core::index::runtime::RuntimeRegistry::get`] |
//! | [`crate::core::index::runtime::RegisterError`] | Registration rejection metadata | Must report winner/loser + policy | [`crate::core::index::runtime::RuntimeRegistry::register_internal`] |
//!
//! # Invariants
//!
//! * Concurrent registrations must be linearizable (see `invariants::test_no_lost_updates`).
//! * Lookup stage precedence must be preserved: ID (`by_id`) then name (`by_name`) then key (`by_key`).
//! * Runtime ordinals must be monotonic across snapshot publications.
//!
//! # Data flow
//!
//! 1. Read path: `get*` loads current snapshot and resolves symbols through staged maps.
//! 2. Write path: `register*` builds candidate entry with the current interner/string contracts.
//! 3. Merge path: runtime register computes collision/precedence outcomes.
//! 4. Publish path: new snapshot is CAS-published or retried if stale.
//!
//! # Lifecycle
//!
//! 1. Startup: `RuntimeRegistry::new` creates a snapshot from builtins.
//! 2. Steady state: readers use lock-free snapshot loads.
//! 3. Extension: runtime registration appends/replaces entries and republishes.
//! 4. Replacement: old snapshots remain alive while referenced by `RegistryRef`.
//!
//! # Concurrency & ordering
//!
//! * Readers are wait-free (`ArcSwap` load + immutable data reads).
//! * Writers are lock-free via CAS retry loop.
//! * Registration ordering is deterministic through precedence and ordinal tie-breakers.
//!
//! # Failure modes & recovery
//!
//! * Duplicate-policy rejection returns [`crate::core::index::runtime::RegisterError::Rejected`].
//! * CAS races are recovered by retrying from the latest snapshot.
//! * Stale refs remain valid because they pin their originating snapshot.
//!
//! # Recipes
//!
//! ## Register a runtime definition
//!
//! * Build or link a `BuildEntry` input.
//! * Call [`crate::core::index::runtime::RuntimeRegistry::register`] or `register_owned`.
//! * Handle `RegisterError::Rejected` when policy blocks replacement.
//!
//! ## Lookup with stable lifetime
//!
//! * Call `get` / `get_sym` / `get_by_id`.
//! * Keep the returned `RegistryRef` as long as data from that snapshot is needed.

use std::cmp::Ordering;
use std::sync::Arc;

use arc_swap::ArcSwap;

use super::snapshot::{RegistryRef, Snapshot, SnapshotGuard};
use super::types::RegistryIndex;
use crate::core::{DenseId, DuplicatePolicy, InternerBuilder, Party, RegistryEntry, Symbol};

mod state;

pub use state::{RegisterError, RuntimeEntry, RuntimeRegistry};

#[cfg(test)]
mod invariants;
