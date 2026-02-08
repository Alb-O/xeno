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
//! 1. [`RegistryBuilder`] ingests static definitions, deduplicates by canonical ID, assigns
//!    dense IDs, and constructs a [`RegistryIndex`].
//! 2. The [`RegistryIndex`] is converted into a [`RuntimeRegistry`], which manages an atomic
//!    [`Snapshot`] of the data.
//! 3. Readers load a [`Snapshot`] and perform O(1) lookups by ID or key, receiving a
//!    [`RegistryRef`] which pins the specific snapshot version.
//! 4. At runtime, [`RuntimeRegistry::register`] builds an extended snapshot and atomically
//!    swaps it, ensuring linearizable updates and lock-free reads.
//!
//! # Precedence Contract
//!
//! Conflict resolution follows a strict precedence hierarchy:
//! 1. Higher [`crate::core::meta::RegistryMeta::priority`] wins.
//! 2. Higher rank [`crate::core::meta::RegistrySource`] wins (Runtime > Crate > Builtin).
//! 3. Deterministic tie-breaker:
//!    - Canonical-ID conflicts: higher (later) ingest ordinal wins.
//!    - Key conflicts (name/key): canonical ID total order wins (via interned symbol).
//!
//! - Enforced in: `cmp_party`, `RegistryEntry::total_order_cmp`
//! - Tested by: `test_source_precedence` (in `invariants.rs`)
//! - Failure symptom: Wrong definition wins a key binding or ID conflict.
//!
//! # 3-Stage Key Model
//!
//! Keys are interned and resolved in three stages, ensuring that canonical identity is
//! never displaced by secondary lookup keys:
//! 1. Stage A: Canonical ID - Immutable identity binding.
//! 2. Stage B: Primary Name - Friendly display name lookup.
//! 3. Stage C: Secondary Keys - User secondary keys and domain-specific lookup keys.
//!
//! # Key Types
//!
//! | Type | Role |
//! |------|------|
//! | [`RuntimeRegistry`] | Atomic container for a specific domain's definitions. |
//! | [`RegistryIndex`] | Immutable, indexed storage produced by the builder. |
//! | [`Snapshot`] | Current state of a registry (tables, maps, interner, key pool). |
//! | [`RegistryRef`] | A pinned handle to an entry, holding its snapshot alive. |
//! | [`RegistryBuilder`] | Pipeline for string interning and ID assignment. |
//! | [`crate::core::domain::DomainSpec`] | Definition of a specific registry domain. |
//!
//! # Concurrency
//!
//! - Reads: wait-free (atomic load of current snapshot).
//! - Writes: lock-free with linearizability (CAS retry loop on registration).
//!
//! # Invariants
//!
//! - Must have unambiguous ID lookup (one winner per ID).
//! - Must maintain deterministic iteration order (sorted by canonical ID).
//! - Must keep owned definitions alive while reachable.
//! - Must provide linearizable writes without lost updates.

mod build;
mod collision;
pub(crate) mod lookup;
pub(crate) mod meta_build;
pub(crate) mod runtime;
pub(crate) mod snapshot;
mod types;

pub use build::{BuildEntry, RegistryBuilder, RegistryMetaRef, StrListRef};
pub(crate) use collision::cmp_party;
pub use collision::{Collision, CollisionKind, DuplicatePolicy, KeyKind, Party, Resolution};
pub use runtime::{RegisterError, RuntimeEntry, RuntimeRegistry};
pub use snapshot::{RegistryRef, Snapshot, SnapshotGuard};
pub use types::RegistryIndex;

#[cfg(test)]
mod invariants;

#[cfg(test)]
pub(crate) mod test_fixtures;

#[cfg(test)]
mod tests;
