use super::capability::Capability;

/// Numeric identifier for an action in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId(pub u32);

impl ActionId {
	/// Represents an invalid action ID.
	pub const INVALID: ActionId = ActionId(u32::MAX);

	/// Returns true if this action ID is valid.
	#[inline]
	pub fn is_valid(self) -> bool {
		self != Self::INVALID
	}

	/// Returns the underlying u32 value.
	#[inline]
	pub fn as_u32(self) -> u32 {
		self.0
	}
}

impl std::fmt::Display for ActionId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if *self == Self::INVALID {
			write!(f, "ActionId(INVALID)")
		} else {
			write!(f, "ActionId({})", self.0)
		}
	}
}

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
	/// Returns the precedence rank of the source (lower is higher precedence).
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

/// Common metadata for all registry item types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryMeta {
	/// Unique identifier (e.g., "xeno-stdlib::move_left").
	pub id: &'static str,
	/// Human-readable name for UI display.
	pub name: &'static str,
	/// Alternative names for command/action lookup.
	pub aliases: &'static [&'static str],
	/// Description for help text.
	pub description: &'static str,
	/// Priority for conflict resolution (higher wins).
	pub priority: i16,
	/// Where this item was defined.
	pub source: RegistrySource,
	/// Capabilities required to execute this item.
	pub required_caps: &'static [Capability],
	/// Bitflags for additional behavior hints.
	pub flags: u32,
}

impl RegistryMeta {
	/// Creates a new RegistryMeta with all fields specified.
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

	/// Creates a minimal RegistryMeta with defaults for optional fields.
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
