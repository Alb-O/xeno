use serde::{Deserialize, Serialize};

/// Raw text object metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectMetaRaw {
	/// Text object name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Primary trigger character (e.g., `"w"`, `"("`).
	pub trigger: String,
	/// Alternate trigger characters.
	pub alt_triggers: Vec<String>,
}

/// Top-level blob containing all text object metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct TextObjectsBlob {
	/// All text object definitions.
	pub text_objects: Vec<TextObjectMetaRaw>,
}
