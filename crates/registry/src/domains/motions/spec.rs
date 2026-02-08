use serde::{Deserialize, Serialize};

pub use crate::defs::spec::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionsSpec {
	pub motions: Vec<MotionSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionSpec {
	pub common: MetaCommonSpec,
}
