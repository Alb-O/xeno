//! Unified precedence and comparison logic for registry entries.
//!
//! # Purpose
//!
//! This module provides a single source of truth for all registry entry comparisons,
//! ensuring consistency across build-time ID duplicates, runtime replacements, and
//! within-stage key conflicts.
//!
//! # Tie-Break Strategies
//!
//! - [`TieBreak::Ordinal`] - Uses ingest ordinal (for canonical ID collisions).
//!   Later ingest wins when priority and source are equal.
//!
//! - [`TieBreak::DefId`] - Uses canonical ID symbol (for key/name conflicts).
//!   Deterministic identity tie-breaker for conflicting keys.

use std::cmp::Ordering;

use crate::core::{Party, RegistryEntry};

/// Tie-break strategy for equal priority and source rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TieBreak {
	/// Use ingest ordinal (later wins).
	Ordinal,
	/// Use canonical ID symbol (deterministic).
	DefId,
}

/// Compares two parties using the global precedence hierarchy.
///
/// Precedence order:
/// 1. Priority (higher wins)
/// 2. Source rank (higher wins: Runtime > Crate > Builtin)
/// 3. Tie-break (ordinal or def_id depending on context)
///
/// # Examples
///
/// For canonical ID duplicates (use ordinal):
/// ```ignore
/// cmp_party(&new_party, &existing_party, TieBreak::Ordinal)
/// ```
///
/// For key conflicts (use def_id):
/// ```ignore
/// cmp_party(&challenger_party, &winner_party, TieBreak::DefId)
/// ```
pub fn cmp_party(a: &Party, b: &Party, tie: TieBreak) -> Ordering {
	a.priority
		.cmp(&b.priority)
		.then_with(|| a.source.rank().cmp(&b.source.rank()))
		.then_with(|| match tie {
			TieBreak::Ordinal => a.ordinal.cmp(&b.ordinal),
			TieBreak::DefId => a.def_id.cmp(&b.def_id),
		})
}

/// Compares two registry entries using the global precedence hierarchy.
///
/// This uses the DefId tie-break strategy (comparing canonical IDs) which is
/// appropriate for within-stage key conflicts.
///
/// Precedence order:
/// 1. Priority (higher wins)
/// 2. Source rank (higher wins: Runtime > Crate > Builtin)
/// 3. Canonical ID symbol (deterministic tie-break)
pub fn cmp_entry<T: RegistryEntry>(a: &T, b: &T) -> Ordering {
	a.priority()
		.cmp(&b.priority())
		.then_with(|| a.source().rank().cmp(&b.source().rank()))
		.then_with(|| a.id().cmp(&b.id()))
}

/// Returns true if party `a` wins over party `b`.
///
/// Convenience wrapper for checking if a challenger should replace an existing entry.
pub fn party_wins(a: &Party, b: &Party, tie: TieBreak) -> bool {
	cmp_party(a, b, tie) == Ordering::Greater
}

/// Returns true if entry `a` wins over entry `b`.
///
/// Convenience wrapper for within-stage conflict resolution.
pub fn entry_wins<T: RegistryEntry>(a: &T, b: &T) -> bool {
	cmp_entry(a, b) == Ordering::Greater
}

// Tests for precedence are in core/index/invariants.rs
// to leverage the existing test infrastructure.
