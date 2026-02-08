//! Collision types and precedence rules.
//!
//! # Role
//!
//! This module defines the vocabulary for conflicts and the canonical precedence rules
//! used to resolve them.

use std::cmp::Ordering;

use crate::core::meta::RegistrySource;
use crate::core::symbol::Symbol;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DuplicatePolicy {
	/// Panic with detailed error message.
	Panic,
	/// Keep the first definition seen for a key.
	FirstWins,
	/// Overwrite with the last definition seen.
	LastWins,
	/// Select winner by priority (higher wins), then source rank, then ID.
	#[default]
	ByPriority,
}

impl DuplicatePolicy {
	/// Returns the appropriate policy based on build configuration.
	#[inline]
	pub fn for_build() -> Self {
		if cfg!(debug_assertions) {
			DuplicatePolicy::Panic
		} else {
			DuplicatePolicy::ByPriority
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Party {
	/// Canonical definition id: meta.id (NOT the conflicting key).
	pub def_id: Symbol,
	pub source: RegistrySource,
	pub priority: i16,
	/// Stable ingest ordinal.
	pub ordinal: u32,
}

/// Compares two parties using the global precedence rules for canonical-ID conflicts.
///
/// Precedence hierarchy:
/// 1. Priority (higher wins)
/// 2. Source (Runtime > Crate > Builtin)
/// 3. Ingest ordinal (higher/later wins)
///
/// Tie-breaking semantics:
/// - At build-time: later ingest wins if priority and source are identical.
/// - At runtime: the new entry is assigned a higher ordinal, so it wins on ties.
pub(crate) fn cmp_party(a: &Party, b: &Party) -> Ordering {
	a.priority
		.cmp(&b.priority)
		.then_with(|| a.source.rank().cmp(&b.source.rank()))
		.then_with(|| a.ordinal.cmp(&b.ordinal))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
	/// Stage A: Immutable unique identifier.
	Canonical,
	/// Stage B: Primary friendly display name.
	PrimaryName,
	/// Stage C: User-defined alias or domain-specific lookup key.
	SecondaryKey,
}

impl std::fmt::Display for KeyKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Canonical => write!(f, "canonical"),
			Self::PrimaryName => write!(f, "primary_name"),
			Self::SecondaryKey => write!(f, "secondary_key"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
	/// Existing binding kept; incoming dropped/ignored.
	KeptExisting,
	/// Existing binding replaced by incoming.
	ReplacedExisting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
	pub registry: &'static str,
	/// The actual lookup key that conflicted: canonical id symbol OR alias symbol.
	pub key: Symbol,
	pub kind: CollisionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollisionKind {
	/// Multiple defs had the same canonical id string; losers dropped before table build.
	DuplicateId {
		winner: Party,
		loser: Party,
		policy: DuplicatePolicy,
	},

	/// A binding attempt for `key` conflicted with an existing binding.
	KeyConflict {
		existing_kind: KeyKind,
		incoming_kind: KeyKind,
		existing: Party,
		incoming: Party,
		resolution: Resolution,
	},
}
