use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionSpec {
	pub common: MetaCommonSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionsSpec {
	pub motions: Vec<MotionSpec>,
}
