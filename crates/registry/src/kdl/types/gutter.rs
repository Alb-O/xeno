use serde::{Deserialize, Serialize};

/// Raw gutter metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct GutterMetaRaw {
	/// Common metadata.
	pub common: super::common::MetaCommonRaw,
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
