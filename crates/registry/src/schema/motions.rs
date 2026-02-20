//! Motion specification schema.
//!
//! Defines motion metadata and runtime binding configuration.


use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionSpec {
	pub common: MetaCommonSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionsSpec {
	#[serde(default)]
	pub motions: Vec<MotionSpec>,
}
