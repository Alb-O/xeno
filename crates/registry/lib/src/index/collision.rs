//! Collision detection for registry items.
//!
//! Tracks when multiple registry items share the same key, helping
//! users identify and resolve registration conflicts.

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

/// Record of a collision between two registry items.
pub struct Collision<T: 'static> {
	/// Type of collision that occurred.
	pub kind: CollisionKind,
	/// The key that caused the collision.
	pub key: String,
	/// The item that won (higher priority).
	pub winner: &'static T,
	/// The item that was shadowed (lower priority).
	pub shadowed: &'static T,
}
