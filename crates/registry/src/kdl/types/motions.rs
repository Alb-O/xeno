use serde::{Deserialize, Serialize};

/// Raw motion metadata extracted from KDL, before handler linking.
#[derive(Debug, Serialize, Deserialize)]
pub struct MotionMetaRaw {
	/// Common metadata.
	pub common: super::common::MetaCommonRaw,
}

/// Top-level blob containing all motion metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct MotionsBlob {
	/// All motion definitions.
	pub motions: Vec<MotionMetaRaw>,
}
