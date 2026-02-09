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
	/// Select winner by priority (higher wins), then source rank, then ordinal.
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

/// Compares two parties using the global precedence rules.
///
/// Precedence hierarchy:
/// 1. Priority (higher wins)
/// 2. Source (Runtime > Crate > Builtin)
/// 3. Ingest ordinal (higher/later wins)
///
/// This is a convenience wrapper around [`super::precedence::cmp_party`].
pub(crate) fn cmp_party(a: &Party, b: &Party) -> Ordering {
	super::precedence::cmp_party(a, b)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Collision {
	pub registry: &'static str,
	/// The actual lookup key that conflicted: canonical id symbol OR alias symbol.
	pub key: Symbol,
	pub kind: CollisionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionKind {
	/// Multiple defs had the same canonical id string; losers dropped before table build.
	DuplicateId {
		winner: Party,
		loser: Party,
		policy: DuplicatePolicy,
	},

	/// A binding attempt for `key` conflicted with an existing binding.
	///
	/// With explicit stage maps, this includes:
	/// - Cross-stage blocks: Attempted Stage B/C binding blocked by a higher-stage winner
	///   (resolution will be [`Resolution::KeptExisting`]).
	/// - Within-stage conflicts: Multiple entries competing for the same key within the
	///   same stage (e.g. two secondary keys), resolved by total order.
	KeyConflict {
		/// The stage of the existing winner for this key.
		existing_kind: KeyKind,
		/// The stage of the incoming entry attempting to bind this key.
		incoming_kind: KeyKind,
		/// The entry currently winning/holding the key.
		existing: Party,
		/// The entry attempting to bind the key.
		incoming: Party,
		/// Whether the incoming entry replaced the existing winner.
		resolution: Resolution,
	},
}

impl Collision {
	/// Provides a stable sort order for collisions.
	pub fn stable_cmp(&self, other: &Self) -> Ordering {
		self.key
			.as_u32()
			.cmp(&other.key.as_u32())
			.then_with(|| self.kind.rank().cmp(&other.kind.rank()))
			.then_with(|| self.kind.winner_ordinal().cmp(&other.kind.winner_ordinal()))
			.then_with(|| self.kind.loser_ordinal().cmp(&other.kind.loser_ordinal()))
	}
}

impl CollisionKind {
	fn rank(&self) -> u8 {
		match self {
			Self::DuplicateId { .. } => 0,
			Self::KeyConflict { incoming_kind, .. } => match incoming_kind {
				KeyKind::Canonical => 1,
				KeyKind::PrimaryName => 2,
				KeyKind::SecondaryKey => 3,
			},
		}
	}

	fn winner_ordinal(&self) -> u32 {
		match self {
			Self::DuplicateId { winner, .. } => winner.ordinal,
			Self::KeyConflict {
				existing,
				incoming,
				resolution,
				..
			} => {
				if *resolution == Resolution::ReplacedExisting {
					incoming.ordinal
				} else {
					existing.ordinal
				}
			}
		}
	}

	fn loser_ordinal(&self) -> u32 {
		match self {
			Self::DuplicateId { loser, .. } => loser.ordinal,
			Self::KeyConflict {
				existing,
				incoming,
				resolution,
				..
			} => {
				if *resolution == Resolution::ReplacedExisting {
					existing.ordinal
				} else {
					incoming.ordinal
				}
			}
		}
	}
}
