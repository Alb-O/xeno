use super::capability::{Capability, CapabilitySet};
use super::symbol::Symbol;

/// Represents where a registry item was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistrySource {
	/// Built directly into the editor.
	Builtin,
	/// Defined in a library crate.
	Crate(&'static str),
	/// Loaded at runtime (e.g., from KDL config files).
	Runtime,
}

impl RegistrySource {
	/// Returns the precedence rank of the source (higher is higher precedence).
	pub const fn rank(self) -> u8 {
		match self {
			Self::Builtin => 0,
			Self::Crate(_) => 1,
			Self::Runtime => 2,
		}
	}
}

impl core::fmt::Display for RegistrySource {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Builtin => write!(f, "builtin"),
			Self::Crate(name) => write!(f, "crate:{name}"),
			Self::Runtime => write!(f, "runtime"),
		}
	}
}

/// Static metadata for const declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryMetaStatic {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub description: &'static str,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: &'static [Capability],
	pub flags: u32,
}

impl RegistryMetaStatic {
	/// Creates a new RegistryMetaStatic with all fields specified.
	#[allow(clippy::too_many_arguments, reason = "constructor for all fields")]
	pub const fn new(
		id: &'static str,
		name: &'static str,
		aliases: &'static [&'static str],
		description: &'static str,
		priority: i16,
		source: RegistrySource,
		required_caps: &'static [Capability],
		flags: u32,
	) -> Self {
		Self {
			id,
			name,
			aliases,
			description,
			priority,
			source,
			required_caps,
			flags,
		}
	}

	/// Creates a minimal RegistryMetaStatic with defaults for optional fields.
	pub const fn minimal(id: &'static str, name: &'static str, description: &'static str) -> Self {
		Self {
			id,
			name,
			aliases: &[],
			description,
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		}
	}
}

/// Metadata string list handle (index range into snapshot alias pool).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SymbolList {
	pub start: u32,
	pub len: u16,
}

/// Common metadata for all registry item types (symbolized for runtime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryMeta {
	/// Unique identifier (interned).
	pub id: Symbol,
	/// Human-readable name for UI display (interned).
	pub name: Symbol,
	/// Description for help text (interned).
	pub description: Symbol,
	/// Alternative names for command/action lookup (interned index range).
	pub aliases: SymbolList,
	/// Priority for conflict resolution (higher wins).
	pub priority: i16,
	/// Where this item was defined.
	pub source: RegistrySource,
	/// Capabilities required to execute this item.
	pub required_caps: CapabilitySet,
	/// Bitflags for additional behavior hints.
	pub flags: u32,
}
