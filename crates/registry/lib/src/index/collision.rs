//! Collision type definitions for diagnostics.
//!
//! Collision detection and invariant enforcement is handled by core registries.
//! This module provides types for diagnostic reporting.

/// Type of collision between registry items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionKind {
	/// Two items share the same unique identifier.
	Id,
	/// Two items share the same display name.
	Name,
	/// Two items share an alias.
	Alias,
	/// Two items share the same trigger character.
	Trigger,
}

impl std::fmt::Display for CollisionKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Id => write!(f, "ID"),
			Self::Name => write!(f, "name"),
			Self::Alias => write!(f, "alias"),
			Self::Trigger => write!(f, "trigger"),
		}
	}
}
