use serde::{Deserialize, Serialize};

/// Raw hook metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct HookMetaRaw {
	/// Common metadata.
	pub common: super::common::MetaCommonRaw,
	/// Event name this hook listens to.
	pub event: String,
}

/// Top-level blob containing all hook metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct HooksBlob {
	/// All hook definitions.
	pub hooks: Vec<HookMetaRaw>,
}
