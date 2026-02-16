//! Centralized registry index infrastructure.
//! Anchor ID: XENO_ANCHOR_REGISTRY_INDEX
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
//!    * Canonical-ID conflicts: higher (later) ingest ordinal wins.
//!    * Key conflicts (name/key): canonical ID total order wins (via interned symbol).
//!
//! * Enforced in: `cmp_party`, `RegistryEntry::total_order_cmp`
//! * Tested by: `test_source_precedence` (in `invariants.rs`)
//! * Failure symptom: Wrong definition wins a key binding or ID conflict.
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
//! | [`crate::db::domain::DomainSpec`] | Definition of a specific registry domain. |
//!
//! # Invariants
//!
//! * Must have unambiguous ID lookup (one winner per ID).
//! * Must maintain deterministic iteration order by dense ID (table index).
//!   Builtins are built in canonical-ID order; runtime appends extend the table in registration order.
//! * Must keep owned definitions alive while reachable.
//! * Must provide linearizable writes without lost updates.
//!
//! # Data flow
//!
//! 1. Domain definitions are pushed into [`RegistryBuilder`].
//! 2. Build phase interns strings, resolves ID/key collisions, and emits [`RegistryIndex`].
//! 3. Runtime wraps the index in [`RuntimeRegistry`] snapshot storage.
//! 4. Readers resolve by key/ID and receive snapshot-pinned [`RegistryRef`] handles.
//! 5. Runtime registrations build extended snapshots and CAS-publish them.
//!
//! # Lifecycle
//!
//! * Build-time bootstrap from builtins/plugins into immutable [`RegistryIndex`].
//! * Runtime steady-state reads from latest snapshot.
//! * Optional runtime mutation via `register`/`register_owned`.
//! * Old snapshots remain valid while referenced by pinned refs.
//!
//! # Concurrency & ordering
//!
//! * Reads are wait-free through atomic snapshot loads.
//! * Writes use CAS loops for lock-free linearizable publication.
//! * Conflict resolution order is deterministic through precedence contract.
//!
//! # Failure modes & recovery
//!
//! * Duplicate-policy rejection returns `RegisterError::Rejected`.
//! * CAS race retries against latest snapshot until publish succeeds or policy rejects.
//! * Collision metadata retains diagnostics when keys/IDs conflict.
//!
//! # Recipes
//!
//! * Add a new registry domain:
//!   1. Define domain `Input/Entry/Id`.
//!   2. Wire builder path and index emission.
//!   3. Reuse this precedence/collision contract for lookup semantics.
//! * Change precedence policy:
//!   1. Update `cmp_party` contract.
//!   2. Update invariant proofs in `invariants.rs`.
//!   3. Validate deterministic winner behavior across build/runtime paths.

mod build;
mod collision;
pub(crate) mod lookup;
pub(crate) mod meta_build;
pub mod precedence;
pub(crate) mod runtime;
pub(crate) mod snapshot;
mod types;
mod util;

pub use build::{BuildCtx, BuildCtxExt, BuildEntry, RegistryBuilder, RegistryMetaRef, StrListRef, StringCollector};
pub(crate) use collision::cmp_party;
pub use collision::{Collision, CollisionKind, DuplicatePolicy, KeyKind, Party, Resolution};
pub use runtime::{RegisterError, RuntimeEntry, RuntimeRegistry};
pub use snapshot::{RegistryRef, Snapshot, SnapshotGuard};
pub use types::RegistryIndex;
pub(crate) use util::u32_index;

#[cfg(test)]
pub(crate) mod test_fixtures;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
