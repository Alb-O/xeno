use serde::{Deserialize, Serialize};

/// Raw command metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandMetaRaw {
	/// Command name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Alternative lookup names (e.g., `"q"` for `"quit"`).
	pub keys: Vec<String>,
}

/// Top-level blob containing all command metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandsBlob {
	/// All command definitions.
	pub commands: Vec<CommandMetaRaw>,
}
