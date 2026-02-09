#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextObjectSpec {
	pub common: MetaCommonSpec,
	pub trigger: String,
	pub alt_triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextObjectsSpec {
	pub text_objects: Vec<TextObjectSpec>,
}
