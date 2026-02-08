use serde::{Deserialize, Serialize};

/// Raw statusline segment metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineMetaRaw {
	/// Common metadata.
	pub common: super::common::MetaCommonRaw,
	/// Position: `"left"` or `"right"`.
	pub position: String,
}

/// Top-level blob containing all statusline segment metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatuslineBlob {
	/// All statusline segment definitions.
	pub segments: Vec<StatuslineMetaRaw>,
}
