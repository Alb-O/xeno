use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSegmentSpec {
	pub common: MetaCommonSpec,
	pub position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSpec {
	pub segments: Vec<StatuslineSegmentSpec>,
}
