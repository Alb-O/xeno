#![cfg_attr(doc, allow(rustdoc::private_intra_doc_links))]
//! Centralized registry index infrastructure.
//!
//! # Purpose
//!
//! The `index` subsystem provides the generic machinery for building, searching, and updating
//! registry definitions. It handles the transition from static definitions (Def) to symbolized
//! runtime entries (Entry), manages string interning, and implements collision resolution.
//!
//! # Mental Model
//!
//! 1. **Build Phase:** [`RegistryBuilder`] ingests static definitions, performs canonical ID
//!    deduplication, assigns dense IDs, and constructs a [`RegistryIndex`].
//! 2. **Publication:** The [`RegistryIndex`] is converted into a [`RuntimeRegistry`], which
//!    manages an atomic [`Snapshot`] of the data.
//! 3. **Consumption:** Readers load a [`Snapshot`] and perform O(1) lookups by ID or key,
//!    receiving a [`RegistryRef`] which pins the specific snapshot version.
//! 4. **Extension:** At runtime, [`RuntimeRegistry::register`] builds an extended snapshot
//!    and atomically swaps it, ensuring linearizable updates and lock-free reads.
//!
//! # Precedence Contract
//!
//! Conflict resolution follows a strict precedence hierarchy:
//! 1. **Priority:** Higher [`crate::core::meta::RegistryMeta::priority`] wins.
//! 2. **Source Rank:** Higher rank [`crate::core::meta::RegistrySource`] wins (Runtime > Crate > Builtin).
//! 3. **Deterministic Tie-breaker:**
//!    - **Canonical-ID conflicts:** Higher (later) ingest ordinal wins.
//!    - **Key conflicts (name/alias):** Canonical ID total order wins (via interned symbol).
//!
//! - Enforced in: [`crate::core::index::collision::cmp_party`], [`crate::core::traits::RegistryEntry::total_order_cmp`]
//! - Tested by: [`crate::core::index::invariants::test_source_precedence`]
//! - Failure symptom: Wrong definition wins a key binding or ID conflict.
//!
//! # Key Types
//!
//! | Type | Role |
//! |------|------|
//! | [`RuntimeRegistry`] | Atomic container for a specific domain's definitions. |
//! | [`RegistryIndex`] | Immutable, indexed storage produced by the builder. |
//! | [`Snapshot`] | Current state of a registry (tables, maps, interner). |
//! | [`RegistryRef`] | A pinned handle to an entry, holding its snapshot alive. |
//! | [`RegistryBuilder`] | Pipeline for string interning and ID assignment. |
//!
//! # Concurrency
//!
//! - **Reads:** Wait-free (atomic load of current snapshot).
//! - **Writes:** Lock-free with linearizability (CAS retry loop on registration).
//!
//! # Invariants
//!
//! - Must have unambiguous ID lookup (one winner per ID).
//!   - Enforced in: [`crate::core::index::build::resolve_id_duplicates`], [`crate::core::index::runtime::RuntimeRegistry::register`].
//!   - Tested by: [`crate::core::index::invariants::test_unambiguous_id_lookup`]
//!   - Failure symptom: Panics or inconsistent lookups.
//!
//! - Must maintain deterministic iteration order (sorted by canonical ID).
//!   - Enforced in: [`crate::core::index::build::resolve_id_duplicates`].
//!   - Tested by: [`crate::core::index::invariants::test_deterministic_iteration`]
//!   - Failure symptom: Iterator order changes unpredictably.
//!
//! - Must keep owned definitions alive while reachable.
//!   - Enforced in: [`crate::core::index::snapshot::RegistryRef`] (holds `Arc<Snapshot>`).
//!   - Tested by: [`crate::core::index::invariants::test_snapshot_liveness_across_swap`]
//!   - Failure symptom: Use-after-free in `RegistryRef` deref.
//!
//! - Must provide linearizable writes without lost updates.
//!   - Enforced in: [`crate::core::index::runtime::RuntimeRegistry::register`] (CAS loop).
//!   - Tested by: [`crate::core::index::invariants::test_no_lost_updates`]
//!   - Failure symptom: Concurrent registrations silently dropped.

mod build;
mod collision;
pub(crate) mod lookup;
pub(crate) mod runtime;
pub(crate) mod snapshot;
mod types;

pub use build::{BuildEntry, RegistryBuilder, RegistryMetaRef};
pub(crate) use collision::cmp_party;
pub use collision::{Collision, CollisionKind, DuplicatePolicy, KeyKind, Party, Resolution};
pub use runtime::{RegisterError, RuntimeEntry, RuntimeRegistry};
pub use snapshot::{RegistryRef, Snapshot, SnapshotGuard};
pub use types::RegistryIndex;

#[cfg(any(test, doc))]
pub(crate) mod invariants;

#[cfg(test)]
pub(crate) mod test_fixtures;

#[cfg(test)]
mod tests;
