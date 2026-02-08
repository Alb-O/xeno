use serde::{Deserialize, Serialize};

/// Raw motion metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct MotionMetaRaw {
	/// Motion name (handler linkage key).
	pub name: String,
	/// Human-readable description.
	pub description: String,
	/// Alternative lookup names.
	pub keys: Vec<String>,
}

/// Top-level blob containing all motion metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct MotionsBlob {
	/// All motion definitions.
	pub motions: Vec<MotionMetaRaw>,
}
