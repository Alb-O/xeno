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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
	Canonical,
	Alias,
}

impl std::fmt::Display for KeyKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Canonical => write!(f, "canonical"),
			Self::Alias => write!(f, "alias"),
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
