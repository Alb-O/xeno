//! Unified precedence and comparison logic for registry entries.
//!
//! # Purpose
//!
//! This module provides a single source of truth for all registry entry comparisons,
//! ensuring consistency across build-time ID duplicates, runtime replacements, and
//! within-stage key conflicts.
//!
//! # Precedence Rules
//!
//! Precedence follows a strict hierarchy (Priority > Source > Ordinal):
//! 1. Priority (higher wins)
//! 2. Source rank (higher wins: Runtime > Crate > Builtin)
//! 3. Ingest ordinal (higher/later wins)
//!
//! This ensures that later registrations (especially from runtime) can override
//! built-ins or earlier registrations deterministically.

use std::cmp::Ordering;

use crate::core::Party;

/// Compares two parties using the global precedence hierarchy.
///
/// Precedence order:
/// 1. Priority (higher wins)
/// 2. Source rank (higher wins: Runtime > Crate > Builtin)
/// 3. Ingest ordinal (higher/later wins)
pub fn cmp_party(a: &Party, b: &Party) -> Ordering {
	a.priority
		.cmp(&b.priority)
		.then_with(|| a.source.rank().cmp(&b.source.rank()))
		.then_with(|| a.ordinal.cmp(&b.ordinal))
}

/// Returns true if party `a` wins over party `b`.
///
/// Convenience wrapper for checking if a challenger should replace an existing entry.
pub fn party_wins(a: &Party, b: &Party) -> bool {
	cmp_party(a, b) == Ordering::Greater
}

// Tests for precedence are in core/index/invariants.rs
// to leverage the existing test infrastructure.
