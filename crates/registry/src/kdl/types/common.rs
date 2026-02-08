use serde::{Deserialize, Serialize};

/// Common metadata shared by all KDL-defined registry entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaCommonRaw {
	/// Canonical name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Short description for HUD (defaults to description if absent).
	pub short_desc: Option<String>,
	/// Alternative lookup names.
	pub keys: Vec<String>,
	/// Conflict resolution priority.
	pub priority: i16,
	/// Required capability names.
	pub caps: Vec<String>,
	/// Behavior hint flags.
	pub flags: u32,
}
