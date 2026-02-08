use serde::{Deserialize, Serialize};

pub use crate::defs::spec::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSpec {
	pub segments: Vec<StatuslineSegmentSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSegmentSpec {
	pub common: MetaCommonSpec,
	pub position: String,
}
