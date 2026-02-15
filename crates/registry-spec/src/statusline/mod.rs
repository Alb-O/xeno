#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

pub const VALID_POSITIONS: &[&str] = &["left", "right", "center"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSegmentSpec {
	pub common: MetaCommonSpec,
	pub position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSpec {
	#[serde(default)]
	pub segments: Vec<StatuslineSegmentSpec>,
}
