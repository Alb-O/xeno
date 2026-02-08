use serde::{Deserialize, Serialize};

/// Raw statusline segment metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineMetaRaw {
	/// Segment name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Position: `"left"` or `"right"`.
	pub position: String,
	/// Rendering priority within position group.
	pub priority: i16,
}

/// Top-level blob containing all statusline segment metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineBlob {
	/// All statusline segment definitions.
	pub segments: Vec<StatuslineMetaRaw>,
}
