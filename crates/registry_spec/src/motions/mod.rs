//! Motion specification schema.
//!
//! Defines motion metadata and runtime binding configuration.

#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionSpec {
	pub common: MetaCommonSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionsSpec {
	#[serde(default)]
	pub motions: Vec<MotionSpec>,
}
