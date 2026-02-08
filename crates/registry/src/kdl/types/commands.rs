use serde::{Deserialize, Serialize};

/// Raw command metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandMetaRaw {
	/// Common metadata.
	pub common: super::common::MetaCommonRaw,
}

/// Top-level blob containing all command metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandsBlob {
	/// All command definitions.
	pub commands: Vec<CommandMetaRaw>,
}
