use serde::{Deserialize, Serialize};

/// Raw hook metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct HookMetaRaw {
	/// Hook name (handler linkage key).
	pub name: String,
	/// Event name this hook listens to.
	pub event: String,
	/// Execution priority (lower = earlier).
	pub priority: i16,
	/// Human-readable description.
	pub description: String,
}

/// Top-level blob containing all hook metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct HooksBlob {
	/// All hook definitions.
	pub hooks: Vec<HookMetaRaw>,
}
