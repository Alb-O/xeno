use serde::{Deserialize, Serialize};

/// Raw gutter metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct GutterMetaRaw {
	/// Gutter name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Rendering priority (lower = further left).
	pub priority: i16,
	/// Width: `"dynamic"` or a fixed integer as string.
	pub width: String,
	/// Whether enabled by default.
	pub enabled: bool,
}

/// Top-level blob containing all gutter metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct GuttersBlob {
	/// All gutter definitions.
	pub gutters: Vec<GutterMetaRaw>,
}
